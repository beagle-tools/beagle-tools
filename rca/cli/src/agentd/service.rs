//! Installing and driving the daemon as an OS service: a launchd
//! `LaunchAgent` on macOS, a `systemd --user` unit on Linux.

use std::path::{Path, PathBuf};
use std::process::Command;

use beagle_utils::paths;

use super::state_dir;

/// The launchd label / service name.
const LABEL: &str = "com.beagle.agentd";

/// The systemd user unit name.
const UNIT: &str = "beagle-agentd.service";

/// The error for every path that needs `$HOME` when it is unset or empty.
const NO_HOME: &str = "HOME is not set";

/// The inputs for a launchd plist.
#[derive(Debug)]
pub struct LaunchdConfig {
    /// The launchd label.
    pub label: String,
    /// Absolute path to the `beagle-agentd` binary.
    pub program: PathBuf,
    /// Directory for the daemon's stdout/stderr logs.
    pub log_dir: PathBuf,
}

/// Renders a launchd `LaunchAgent` plist that starts the daemon on login and
/// restarts it on crash.
#[must_use]
pub fn render_plist(config: &LaunchdConfig) -> String {
    let out_log = config.log_dir.join("agentd.out.log");
    let err_log = config.log_dir.join("agentd.err.log");
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{program}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>ProcessType</key>
    <string>Background</string>
    <key>StandardOutPath</key>
    <string>{out}</string>
    <key>StandardErrorPath</key>
    <string>{err}</string>
</dict>
</plist>
"#,
        label = config.label,
        program = config.program.display(),
        out = out_log.display(),
        err = err_log.display(),
    )
}

/// Renders a `systemd --user` service unit that restarts the daemon on crash
/// and starts it on login (`WantedBy=default.target`).
#[must_use]
pub fn render_unit(program: &Path) -> String {
    format!(
        "[Unit]\n\
         Description=beagle agent daemon (beagle-agentd)\n\
         After=default.target\n\
         \n\
         [Service]\n\
         Type=simple\n\
         ExecStart={program}\n\
         Restart=always\n\
         RestartSec=5\n\
         \n\
         [Install]\n\
         WantedBy=default.target\n",
        program = program.display()
    )
}

/// Installs and enables the daemon as an OS service (launchd on macOS,
/// `systemd --user` on Linux) so it runs on login and restarts on crash.
///
/// # Errors
/// Returns a message on an unsupported platform, if `beagle-agentd` cannot be
/// found, or if writing the unit / running the service manager fails.
pub fn install() -> Result<String, String> {
    dispatch("install", launchd_install, systemd_install)
}

/// Disables and removes the daemon service.
///
/// # Errors
/// Returns a message on an unsupported platform or if removal fails.
pub fn uninstall() -> Result<String, String> {
    dispatch("uninstall", launchd_uninstall, systemd_uninstall)
}

/// (Re)starts the installed service.
///
/// # Errors
/// Returns a message on an unsupported platform, if the service is not
/// installed, or if the service manager fails.
pub fn start() -> Result<String, String> {
    dispatch("start", launchd_start, systemd_start)
}

/// Stops the service.
///
/// # Errors
/// Returns a message on an unsupported platform or if the service manager fails.
pub fn stop() -> Result<String, String> {
    dispatch("stop", launchd_stop, systemd_stop)
}

/// Picks the macOS or Linux backend for an action, or errors on an unsupported
/// platform.
fn dispatch(
    action: &str,
    macos: fn() -> Result<String, String>,
    linux: fn() -> Result<String, String>,
) -> Result<String, String> {
    if cfg!(target_os = "macos") {
        macos()
    } else if cfg!(target_os = "linux") {
        linux()
    } else {
        Err(format!(
            "`beagle agent {action}` is only supported on macOS and Linux"
        ))
    }
}

// ---- macOS (launchd) ----

fn launchd_install() -> Result<String, String> {
    let program = find_agentd()?;
    let log_dir = state_dir().ok_or(NO_HOME)?.join("logs");
    std::fs::create_dir_all(&log_dir).map_err(|err| format!("creating log dir: {err}"))?;
    let config = LaunchdConfig {
        label: LABEL.to_string(),
        program,
        log_dir,
    };
    let path = plist_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("creating LaunchAgents dir: {err}"))?;
    }
    std::fs::write(&path, render_plist(&config))
        .map_err(|err| format!("writing plist {}: {err}", path.display()))?;

    let domain = gui_domain()?;
    // Reload cleanly if a previous copy was loaded.
    let _ = launchctl(&["bootout", &format!("{domain}/{LABEL}")]);
    launchctl(&["bootstrap", &domain, &path.to_string_lossy()])?;
    Ok(format!(
        "installed launchd agent {LABEL}\n  plist: {}\n  starts on login, restarts on crash",
        path.display()
    ))
}

fn launchd_uninstall() -> Result<String, String> {
    let domain = gui_domain()?;
    let _ = launchctl(&["bootout", &format!("{domain}/{LABEL}")]);
    let path = plist_path()?;
    remove_if_present(&path, "plist")?;
    Ok(format!("uninstalled launchd agent {LABEL}"))
}

fn launchd_start() -> Result<String, String> {
    let path = plist_path()?;
    if !path.is_file() {
        return Err("agent is not installed; run `beagle agent install` first".to_string());
    }
    launchctl(&["bootstrap", &gui_domain()?, &path.to_string_lossy()])?;
    Ok(format!("started {LABEL}"))
}

fn launchd_stop() -> Result<String, String> {
    launchctl(&["bootout", &format!("{}/{LABEL}", gui_domain()?)])?;
    Ok(format!("stopped {LABEL}"))
}

/// The launchd plist path: `~/Library/LaunchAgents/<label>.plist`.
fn plist_path() -> Result<PathBuf, String> {
    let home = paths::home_dir().ok_or(NO_HOME)?;
    Ok(home
        .join("Library/LaunchAgents")
        .join(format!("{LABEL}.plist")))
}

/// The launchd GUI domain target for the current user (`gui/<uid>`).
fn gui_domain() -> Result<String, String> {
    let output = Command::new("id")
        .arg("-u")
        .output()
        .map_err(|err| format!("running id: {err}"))?;
    if !output.status.success() {
        return Err("could not determine the current uid".to_string());
    }
    let uid = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(format!("gui/{uid}"))
}

/// Runs `launchctl args...`, mapping a non-zero exit to an error.
fn launchctl(args: &[&str]) -> Result<(), String> {
    run_checked("launchctl", &[], args)
}

// ---- Linux (systemd --user) ----

fn systemd_install() -> Result<String, String> {
    let program = find_agentd()?;
    let path = unit_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("creating systemd user dir: {err}"))?;
    }
    std::fs::write(&path, render_unit(&program))
        .map_err(|err| format!("writing unit {}: {err}", path.display()))?;
    systemctl(&["daemon-reload"])?;
    systemctl(&["enable", "--now", UNIT])?;
    Ok(format!(
        "installed systemd --user unit {UNIT}\n  unit: {}\n  starts on login, restarts on crash\n  tip: `loginctl enable-linger $USER` keeps it running without an active login",
        path.display()
    ))
}

fn systemd_uninstall() -> Result<String, String> {
    let _ = systemctl(&["disable", "--now", UNIT]);
    let path = unit_path()?;
    remove_if_present(&path, "unit")?;
    let _ = systemctl(&["daemon-reload"]);
    Ok(format!("uninstalled systemd --user unit {UNIT}"))
}

fn systemd_start() -> Result<String, String> {
    let path = unit_path()?;
    if !path.is_file() {
        return Err("agent is not installed; run `beagle agent install` first".to_string());
    }
    systemctl(&["start", UNIT])?;
    Ok(format!("started {UNIT}"))
}

fn systemd_stop() -> Result<String, String> {
    systemctl(&["stop", UNIT])?;
    Ok(format!("stopped {UNIT}"))
}

/// The systemd user-unit path: `$XDG_CONFIG_HOME/systemd/user/<unit>`, else
/// `~/.config/systemd/user/<unit>`.
fn unit_path() -> Result<PathBuf, String> {
    let config = paths::xdg_config_home().ok_or(NO_HOME)?;
    Ok(config.join("systemd/user").join(UNIT))
}

/// Runs `systemctl --user args...`, mapping a non-zero exit to an error.
fn systemctl(args: &[&str]) -> Result<(), String> {
    run_checked("systemctl", &["--user"], args)
}

// ---- shared ----

/// Runs `program prefix... args...`, mapping a spawn failure or non-zero exit
/// to a message that names the full invocation and its stderr.
fn run_checked(program: &str, prefix: &[&str], args: &[&str]) -> Result<(), String> {
    let output = Command::new(program)
        .args(prefix)
        .args(args)
        .output()
        .map_err(|err| format!("running {program}: {err}"))?;
    if output.status.success() {
        return Ok(());
    }
    let invocation = prefix
        .iter()
        .chain(args)
        .fold(program.to_string(), |acc, arg| format!("{acc} {arg}"));
    Err(format!(
        "{invocation} failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    ))
}

/// Deletes the service file at `path`; already gone counts as success.
fn remove_if_present(path: &Path, what: &str) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(format!("removing {what} {}: {err}", path.display())),
    }
}

/// Resolves the `beagle-agentd` binary: `$BEAGLE_AGENTD`, else an executable
/// on the augmented `PATH` (`PATH` plus Homebrew, `~/.local/bin`, `~/bin`, and
/// the login shell's `PATH`).
fn find_agentd() -> Result<PathBuf, String> {
    if let Some(explicit) = paths::env_path("BEAGLE_AGENTD") {
        return Ok(explicit);
    }
    beagle_utils::process::which("beagle-agentd").ok_or_else(|| {
        "beagle-agentd not found on PATH; build it (`cargo build -p beagle-agentd`) and put it on PATH, or set BEAGLE_AGENTD".to_string()
    })
}
