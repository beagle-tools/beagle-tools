//! The Linux backend: a `systemd --user` unit. Split from `service.rs`.

use std::path::PathBuf;

use beagle_utils::paths;

use super::{
    find_agentd, install_store_root, remove_if_present, render_unit, run_checked, NO_HOME, UNIT,
};

// ---- Linux (systemd --user) ----

pub(super) fn systemd_install() -> Result<String, String> {
    let program = find_agentd()?;
    let path = unit_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("creating systemd user dir: {err}"))?;
    }
    std::fs::write(
        &path,
        render_unit(
            &program,
            &beagle_utils::process::augmented_path(),
            install_store_root().as_deref(),
        ),
    )
    .map_err(|err| format!("writing unit {}: {err}", path.display()))?;
    systemctl(&["daemon-reload"])?;
    systemctl(&["enable", "--now", UNIT])?;
    Ok(format!(
        "installed systemd --user unit {UNIT}\n  unit: {}\n  starts on login, restarts on crash\n  tip: `loginctl enable-linger $USER` keeps it running without an active login",
        path.display()
    ))
}

pub(super) fn systemd_uninstall() -> Result<String, String> {
    let _ = systemctl(&["disable", "--now", UNIT]);
    let path = unit_path()?;
    remove_if_present(&path, "unit")?;
    let _ = systemctl(&["daemon-reload"]);
    Ok(format!("uninstalled systemd --user unit {UNIT}"))
}

pub(super) fn systemd_start() -> Result<String, String> {
    let path = unit_path()?;
    if !path.is_file() {
        return Err("agent is not installed; run `beagle rca agent install` first".to_string());
    }
    systemctl(&["start", UNIT])?;
    Ok(format!("started {UNIT}"))
}

pub(super) fn systemd_stop() -> Result<String, String> {
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
