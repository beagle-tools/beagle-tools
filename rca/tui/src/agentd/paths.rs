//! The daemon's well-known filesystem locations — the single source of truth
//! for the CLI, `beagle-agent`, `beagle-agentd`, and the desktop app, so a
//! client always looks where the daemon actually listens and writes.
//!
//! Empty environment variables count as unset (see [`beagle_utils::paths`]).

use std::ffi::OsStr;
use std::path::PathBuf;

use beagle_utils::paths;

/// The daemon's control-socket path: `$XDG_RUNTIME_DIR/beagle-agentd.sock`,
/// else `~/.local/state/beagle/agentd.sock` (note: a fixed `~/.local/state`,
/// not `$XDG_STATE_HOME`), else a relative `beagle-agentd.sock` when `$HOME`
/// is unset too.
#[must_use]
pub fn socket_path() -> PathBuf {
    if let Some(dir) = paths::xdg_runtime_dir() {
        return dir.join("beagle-agentd.sock");
    }
    if let Some(home) = paths::home_dir() {
        return home.join(".local/state/beagle/agentd.sock");
    }
    PathBuf::from("beagle-agentd.sock")
}

/// The daemon's state directory (job databases, worktrees, run registry,
/// logs): `$XDG_STATE_HOME/beagle-agent`, else `~/.local/state/beagle-agent`.
/// `None` only when neither variable is set.
#[must_use]
pub fn state_dir() -> Option<PathBuf> {
    paths::xdg_state_home().map(|dir| dir.join("beagle-agent"))
}

/// The agents config file the daemon reads:
/// `$XDG_CONFIG_HOME/beagle/agents.toml`, else `~/.config/beagle/agents.toml`.
/// `None` only when neither variable is set.
#[must_use]
pub fn config_path() -> Option<PathBuf> {
    config_path_from(
        std::env::var_os("XDG_CONFIG_HOME").as_deref(),
        std::env::var_os("HOME").as_deref(),
    )
}

/// Pure form of [`config_path`] taking the raw `$XDG_CONFIG_HOME` and `$HOME`
/// values, so the resolution rules are testable without touching the process
/// environment.
#[must_use]
pub fn config_path_from(xdg_config_home: Option<&OsStr>, home: Option<&OsStr>) -> Option<PathBuf> {
    paths::xdg_config_home_from(xdg_config_home, home)
        .map(|dir| dir.join("beagle").join("agents.toml"))
}
