//! `beagle serve install|uninstall|status`: the server as a launchd agent
//! (`~/Library/LaunchAgents/com.beagle.serve.plist`), started at login and
//! restarted if it exits. macOS only: the server is for a Mac on a tailnet.
//!
//! Every `launchctl` / `id` call is bounded by [`TOOL_TIMEOUT`].

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use beagle_utils::process::output_with_timeout;

use crate::error::{Error, Result};

/// The launchd label.
pub const LABEL: &str = "com.beagle.serve";

/// Bound on one `launchctl` or `id` call.
pub const TOOL_TIMEOUT: Duration = Duration::from_secs(15);

/// Seconds launchd waits before restarting a server that exited, so a
/// missing certificate doesn't become a tight restart loop.
pub const THROTTLE_SECS: u32 = 30;

/// Store-location variables copied from the installing shell into the
/// agent's environment, so the service reads the stores the shell's
/// `beagle` commands do.
pub const PASSED_ENV: [&str; 12] = [
    "BEAGLE_SCOUT_ROOT",
    "BEAGLE_RECAP_ROOT",
    "BEAGLE_STANDUP_ROOT",
    "BEAGLE_RCA_ROOT",
    "BEAGLE_TANDEM_ROOT",
    "SCOUT_ROOT",
    "RECAP_ROOT",
    "STANDUP_ROOT",
    "BEAGLE_CONFIG",
    "TANDEM_TUI_DB",
    "XDG_CONFIG_HOME",
    "XDG_DATA_HOME",
];

/// Everything the plist says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plist {
    /// The `beagle` binary.
    pub program: PathBuf,
    /// Where stdout/stderr go (`serve.out.log`, `serve.err.log`).
    pub log_dir: PathBuf,
    /// The working directory (the home directory).
    pub working_dir: PathBuf,
    /// Environment: `PATH` plus [`PASSED_ENV`] values that were set.
    pub env: Vec<(String, String)>,
}

fn xml(raw: &str) -> String {
    raw.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// The plist launchd loads: `beagle serve` at login, kept alive, throttled.
#[must_use]
pub fn render_plist(p: &Plist) -> String {
    let mut env = String::new();
    for (k, v) in &p.env {
        env.push_str("        <key>");
        env.push_str(&xml(k));
        env.push_str("</key>\n        <string>");
        env.push_str(&xml(v));
        env.push_str("</string>\n");
    }
    let path = |name: &str| xml(&p.log_dir.join(name).display().to_string());
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{program}</string>
        <string>serve</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>ThrottleInterval</key>
    <integer>{THROTTLE_SECS}</integer>
    <key>ProcessType</key>
    <string>Background</string>
    <key>WorkingDirectory</key>
    <string>{workdir}</string>
    <key>EnvironmentVariables</key>
    <dict>
{env}    </dict>
    <key>StandardOutPath</key>
    <string>{out}</string>
    <key>StandardErrorPath</key>
    <string>{err}</string>
</dict>
</plist>
"#,
        program = xml(&p.program.display().to_string()),
        workdir = xml(&p.working_dir.display().to_string()),
        out = path("serve.out.log"),
        err = path("serve.err.log"),
    )
}

/// What launchd says about the agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// No plist.
    NotInstalled,
    /// A plist, but launchd has not loaded it.
    NotLoaded(PathBuf),
    /// Loaded; `pid` when it is running right now.
    Loaded {
        /// The plist.
        plist: PathBuf,
        /// launchd's `state` (`running`, `waiting`, …).
        state: String,
        /// The server's pid while it runs.
        pid: Option<u32>,
    },
}

impl Status {
    /// One line for `beagle serve status` and `beagle doctor`.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::NotInstalled => {
                "beagle serve: not installed as a service (`beagle serve install`)".to_owned()
            }
            Self::NotLoaded(plist) => format!(
                "beagle serve: installed ({}) but not loaded — run `beagle serve install` again",
                plist.display()
            ),
            Self::Loaded {
                state,
                pid: Some(pid),
                ..
            } => format!("beagle serve: {state} (pid {pid})"),
            Self::Loaded { state, .. } => format!(
                "beagle serve: loaded but {state} — see the log: {}",
                log_dir().map_or_else(
                    |_| "?".to_owned(),
                    |d| d.join("serve.err.log").display().to_string()
                )
            ),
        }
    }

    /// Whether the server is up.
    #[must_use]
    pub fn is_running(&self) -> bool {
        matches!(self, Self::Loaded { pid: Some(_), .. })
    }
}

/// `state` and `pid` from `launchctl print gui/<uid>/<label>` output.
#[must_use]
pub fn parse_print(stdout: &str) -> (Option<String>, Option<u32>) {
    let field = |name: &str| {
        stdout.lines().find_map(|line| {
            let (key, value) = line.trim().split_once(" = ")?;
            (key == name).then(|| value.trim().to_owned())
        })
    };
    (field("state"), field("pid").and_then(|p| p.parse().ok()))
}

fn home() -> Result<PathBuf> {
    beagle_utils::paths::home_dir().ok_or(Error::NoConfigDir)
}

/// `~/Library/LaunchAgents/com.beagle.serve.plist`.
///
/// # Errors
/// [`Error::NoConfigDir`] without a home directory.
pub fn plist_path() -> Result<PathBuf> {
    Ok(home()?
        .join("Library/LaunchAgents")
        .join(format!("{LABEL}.plist")))
}

/// `$XDG_STATE_HOME/beagle/serve` (or `~/.local/state/beagle/serve`).
///
/// # Errors
/// [`Error::NoConfigDir`] without a home directory.
pub fn log_dir() -> Result<PathBuf> {
    beagle_utils::paths::xdg_state_home()
        .map(|d| d.join("beagle/serve"))
        .ok_or(Error::NoConfigDir)
}

fn tool(program: &str, args: &[&str]) -> Result<std::process::Output> {
    output_with_timeout(Command::new(program).args(args), TOOL_TIMEOUT)
        .map_err(|e| Error::Service(format!("`{program} {}`: {e}", args.join(" "))))
}

fn checked(program: &str, args: &[&str]) -> Result<()> {
    let out = tool(program, args)?;
    if out.status.success() {
        return Ok(());
    }
    Err(Error::Service(format!(
        "`{program} {}` failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&out.stderr).trim()
    )))
}

/// `gui/<uid>`, the current user's launchd domain.
fn domain() -> Result<String> {
    let out = tool("id", &["-u"])?;
    let uid = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    if !out.status.success() || uid.is_empty() {
        return Err(Error::Service("could not determine the current uid".into()));
    }
    Ok(format!("gui/{uid}"))
}

fn macos_only() -> Result<()> {
    if cfg!(target_os = "macos") {
        Ok(())
    } else {
        Err(Error::Service(
            "the background service is macOS-only (launchd); run `beagle serve` under your own supervisor".into(),
        ))
    }
}

/// The `beagle` to run: this binary, named by its `PATH` entry when that
/// entry resolves to this same file (a Homebrew symlink survives upgrades,
/// the versioned path it points at does not). A different `beagle` on
/// `PATH` (an old standalone one) is never chosen.
fn program() -> Result<PathBuf> {
    let this = std::env::current_exe().map_err(Error::io("current executable"))?;
    let same = |path: &Path| match (path.canonicalize(), this.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    };
    Ok(beagle_utils::process::which("beagle")
        .filter(|found| same(found))
        .unwrap_or(this))
}

/// Writes the plist and (re)loads it, which also restarts a running
/// server — so re-running `install` picks up a new token or certificate.
///
/// # Errors
/// [`Error::Service`] off macOS or when `launchctl` fails; [`Error::Io`]
/// when the plist or log directory can't be written.
pub fn install() -> Result<String> {
    macos_only()?;
    let log_dir = log_dir()?;
    std::fs::create_dir_all(&log_dir).map_err(Error::io(&log_dir))?;
    let mut env = vec![("PATH".to_owned(), beagle_utils::process::augmented_path())];
    env.extend(PASSED_ENV.iter().filter_map(|k| {
        let value = std::env::var(k).ok().filter(|v| !v.is_empty())?;
        Some(((*k).to_owned(), value))
    }));
    let plist = Plist {
        program: program()?,
        log_dir,
        working_dir: home()?,
        env,
    };
    let path = plist_path()?;
    write_plist(&path, &render_plist(&plist))?;
    let domain = domain()?;
    // Not loaded yet is the normal first-install case, not an error.
    let _ = tool("launchctl", &["bootout", &format!("{domain}/{LABEL}")]);
    checked(
        "launchctl",
        &["bootstrap", &domain, &path.to_string_lossy()],
    )?;
    Ok(format!(
        "installed launchd agent {LABEL}\n  plist: {}\n  logs:  {}\n  starts at login, restarts if it exits",
        path.display(),
        plist.log_dir.display()
    ))
}

fn write_plist(path: &Path, text: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(Error::io(parent))?;
    }
    beagle_utils::fs::write_atomic(path, text.as_bytes()).map_err(Error::io(path))
}

/// Unloads the agent and removes its plist.
///
/// # Errors
/// [`Error::Service`] off macOS; [`Error::Io`] if the plist can't be removed.
pub fn uninstall() -> Result<String> {
    macos_only()?;
    // Already unloaded is fine: uninstall is idempotent.
    let _ = tool("launchctl", &["bootout", &format!("{}/{LABEL}", domain()?)]);
    let path = plist_path()?;
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(format!("uninstalled launchd agent {LABEL}")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Ok(format!("{LABEL} was not installed"))
        }
        Err(e) => Err(Error::io(&path)(e)),
    }
}

/// What launchd says about the agent (read-only).
///
/// # Errors
/// [`Error::Service`] off macOS or when `id` fails.
pub fn status() -> Result<Status> {
    macos_only()?;
    let plist = plist_path()?;
    if !plist.is_file() {
        return Ok(Status::NotInstalled);
    }
    let out = tool("launchctl", &["print", &format!("{}/{LABEL}", domain()?)])?;
    if !out.status.success() {
        return Ok(Status::NotLoaded(plist));
    }
    let (state, pid) = parse_print(&String::from_utf8_lossy(&out.stdout));
    Ok(Status::Loaded {
        plist,
        state: state.unwrap_or_else(|| "unknown".to_owned()),
        pid,
    })
}

/// Restarts the loaded agent so it rereads `serve.toml` (after a token
/// rotation). `Ok(false)` when the agent is not loaded.
///
/// # Errors
/// [`Error::Service`] when `launchctl kickstart` fails.
pub fn restart_if_loaded() -> Result<bool> {
    if !matches!(status()?, Status::Loaded { .. }) {
        return Ok(false);
    }
    checked(
        "launchctl",
        &["kickstart", "-k", &format!("{}/{LABEL}", domain()?)],
    )?;
    Ok(true)
}

/// Starts the server for `beagle server-mode on`: installs and loads the
/// agent when it isn't loaded, starts a loaded one that isn't running, and
/// leaves a running one alone. Returns what it did.
///
/// # Errors
/// As [`install`] and [`status`]; [`Error::Service`] when `kickstart` fails.
pub fn start() -> Result<String> {
    match status()? {
        Status::NotInstalled | Status::NotLoaded(_) => install(),
        Status::Loaded { pid: Some(_), .. } => Ok("already running".to_owned()),
        Status::Loaded { .. } => {
            checked(
                "launchctl",
                &["kickstart", &format!("{}/{LABEL}", domain()?)],
            )?;
            Ok("started".to_owned())
        }
    }
}

/// Stops the server and unloads the agent (`bootout`, so launchd doesn't
/// restart it), keeping its plist, token, phones and settings. `Ok(false)`
/// when it wasn't loaded.
///
/// # Errors
/// As [`status`]; [`Error::Service`] when `bootout` fails.
pub fn stop() -> Result<bool> {
    if !matches!(status()?, Status::Loaded { .. }) {
        return Ok(false);
    }
    checked("launchctl", &["bootout", &format!("{}/{LABEL}", domain()?)])?;
    Ok(true)
}

#[cfg(test)]
#[path = "tests/service.rs"]
mod tests;
