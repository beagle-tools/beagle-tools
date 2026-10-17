//! `agentctl daemon install|restart|uninstall` — run agentd under launchd
//! from the installed binaries alone (a brew install has no repo checkout,
//! so the unit template, icons and notifier builder are embedded; see
//! [`crate::launchd`]).

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use agent_core::paths;
use anyhow::{bail, Context, Result};
use beagle_utils::process::which;

use crate::launchd::{
    parse_pid, render_plist, resolve_agentd, BUILD_NOTIFIER_SH, LABEL, NOTIFY_ICONS,
};

/// How long to wait for launchd to report a running agentd.
const START_TIMEOUT: Duration = Duration::from_secs(12);

/// Render the unit; with `print`, only show it. Otherwise write it to
/// `~/Library/LaunchAgents`, unpack the notification icons, build the
/// branded notifiers when terminal-notifier is installed, and (re)load the
/// unit so the daemon is running when this returns.
pub fn install(print: bool) -> Result<()> {
    let agentd = locate_agentd()?;
    let log_dir = home()?.join("Library/Logs/agentd");
    let plist = render_plist(&agentd, &log_dir);
    if print {
        print!("{plist}");
        return Ok(());
    }
    require_macos()?;
    std::fs::create_dir_all(&log_dir)
        .with_context(|| format!("create log dir {}", log_dir.display()))?;
    let target = plist_path()?;
    if let Some(dir) = target.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
    }
    beagle_utils::fs::write_atomic(&target, plist.as_bytes())
        .with_context(|| format!("write {}", target.display()))?;
    println!("wrote {} (agentd: {})", target.display(), agentd.display());

    install_notifiers()?;

    let domain = gui_domain()?;
    // Replacing a loaded unit needs a bootout first; "not loaded" is fine.
    let _ = launchctl(&["bootout", &format!("{domain}/{LABEL}")]);
    run_launchctl(&["bootstrap", &domain, &target.to_string_lossy()])?;
    let _ = launchctl(&["kickstart", &format!("{domain}/{LABEL}")]);
    report_running(&domain)
}

/// Restart the daemon so a freshly upgraded binary takes over. Required
/// after every upgrade: manifests use `deny_unknown_fields`, so an old
/// daemon silently drops agents that use a field it predates.
pub fn restart() -> Result<()> {
    require_macos()?;
    let domain = gui_domain()?;
    if launchctl(&["print", &format!("{domain}/{LABEL}")]).is_err() {
        bail!("{LABEL} is not loaded — run `agentctl daemon install` first");
    }
    run_launchctl(&["kickstart", "-k", &format!("{domain}/{LABEL}")])?;
    report_running(&domain)
}

/// Stop the daemon and remove its unit. Runs in flight keep going in tmux;
/// the store, logs and notifier apps are left in place.
pub fn uninstall() -> Result<()> {
    require_macos()?;
    let domain = gui_domain()?;
    let _ = launchctl(&["bootout", &format!("{domain}/{LABEL}")]);
    let target = plist_path()?;
    match std::fs::remove_file(&target) {
        Ok(()) => println!("removed {}", target.display()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            println!("{} was not installed", target.display());
        }
        Err(e) => return Err(e).with_context(|| format!("remove {}", target.display())),
    }
    Ok(())
}

fn require_macos() -> Result<()> {
    if cfg!(target_os = "macos") {
        Ok(())
    } else {
        bail!("`agentctl daemon` manages a launchd unit and only works on macOS — run `agentd` under your own service manager instead")
    }
}

fn home() -> Result<PathBuf> {
    beagle_utils::paths::home_dir().context("$HOME is not set")
}

fn plist_path() -> Result<PathBuf> {
    Ok(home()?.join(format!("Library/LaunchAgents/{LABEL}.plist")))
}

fn locate_agentd() -> Result<PathBuf> {
    let exe = std::env::current_exe().ok();
    resolve_agentd(exe.as_deref(), which("agentd"), Path::is_file).context(
        "could not find the agentd binary next to agentctl or on PATH — \
         install it (brew install beagle-tools/tap/beagle-workflows, or `make install`)",
    )
}

/// Unpack the stock icons into `<state>/assets/notify/` (where agentd looks
/// when the manifests repo has none) and build one branded notifier app per
/// icon. Without terminal-notifier, notifications fall back to osascript.
fn install_notifiers() -> Result<()> {
    let state = paths::state_dir().context("resolve agentd state dir")?;
    let icons = state.join("assets/notify");
    std::fs::create_dir_all(&icons).with_context(|| format!("create {}", icons.display()))?;
    for (name, bytes) in NOTIFY_ICONS {
        let path = icons.join(name);
        beagle_utils::fs::write_atomic(&path, bytes)
            .with_context(|| format!("write {}", path.display()))?;
    }
    if which("terminal-notifier").is_none() {
        println!(
            "terminal-notifier not found: notifications will have no icon \
             (brew install terminal-notifier, then re-run `agentctl daemon install`)"
        );
        return Ok(());
    }
    let status = beagle_utils::process::command("bash")
        .args(["-c", BUILD_NOTIFIER_SH, "build-notifier"])
        .env("AGENTD_NOTIFY_ICONS_DIR", &icons)
        .env("AGENTD_STATE_DIR", &state)
        .status()
        .context("run the notifier builder")?;
    if !status.success() {
        // Icons are cosmetic; a failed build must not block the daemon.
        eprintln!("warning: building the notifier apps failed ({status}) — notifications fall back to osascript");
    }
    Ok(())
}

/// `gui/<uid>`, the per-user launchd domain.
fn gui_domain() -> Result<String> {
    let out = Command::new("id")
        .arg("-u")
        .output()
        .context("run `id -u`")?;
    let uid = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    if !out.status.success() || uid.is_empty() {
        bail!("`id -u` failed — cannot address the user's launchd domain");
    }
    Ok(format!("gui/{uid}"))
}

/// Run launchctl quietly, returning its stdout on success.
fn launchctl(args: &[&str]) -> Result<String> {
    let out = Command::new("launchctl")
        .args(args)
        .stdin(Stdio::null())
        .output()
        .context("run launchctl")?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        bail!(
            "launchctl {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        )
    }
}

fn run_launchctl(args: &[&str]) -> Result<()> {
    launchctl(args).map(|_| ())
}

/// Wait for launchd to report a pid. kickstart can return 0 for a process
/// that dies a moment later, so its exit code alone proves nothing.
fn report_running(domain: &str) -> Result<()> {
    let service = format!("{domain}/{LABEL}");
    let deadline = std::time::Instant::now() + START_TIMEOUT;
    loop {
        if let Some(pid) = launchctl(&["print", &service])
            .ok()
            .as_deref()
            .and_then(parse_pid)
        {
            println!("agentd running (pid {pid})");
            return Ok(());
        }
        if std::time::Instant::now() >= deadline {
            bail!(
                "agentd did not come up — runs will sit Pending. \
                 Check ~/Library/Logs/agentd/agentd.err.log, then `agentctl daemon restart`"
            );
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}
