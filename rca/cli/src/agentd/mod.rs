//! Managing the `beagle-agentd` daemon from the CLI: install/uninstall it as an
//! OS service, start/stop it, and query its status.
//!
//! On macOS the service is a launchd `LaunchAgent` (`KeepAlive` + `RunAtLoad`);
//! on Linux it is a `systemd --user` unit (`Restart=always`,
//! `WantedBy=default.target`). Either way it starts on login and restarts on
//! crash. Status talks to a running daemon directly over its loopback control
//! socket (newline-JSON), so the CLI needs no dependency on the daemon crate —
//! it just speaks the wire format.
//!
//! Submodules: `paths` (the canonical socket/state/config locations, shared
//! with the daemon crates and the desktop app), `service` (launchd/systemd
//! install and lifecycle), and `client` (control-socket requests and the TUI
//! status poller).

mod client;
mod paths;
mod service;

use std::path::PathBuf;

use serde::Deserialize;

pub use client::{
    fetch_status, reload_config, spawn_status_poller, start_agent, status, stop_agent,
};
pub use paths::{config_path, config_path_from, socket_path, state_dir};
pub use service::{install, render_plist, render_unit, start, stop, uninstall, LaunchdConfig};

#[cfg(test)]
use client::{format_snapshot, parse_status};

/// A snapshot of the whole daemon, decoded from a `get-status` response. Mirrors
/// the daemon's `DaemonStatus` wire type (the CLI can't depend on the daemon
/// crate, so it re-declares the shape it reads).
#[derive(Debug, Clone, Deserialize)]
pub struct DaemonSnapshot {
    /// The daemon version.
    pub version: String,
    /// One entry per configured agent.
    pub agents: Vec<AgentSnapshot>,
}

/// One agent's status within a [`DaemonSnapshot`].
#[derive(Debug, Clone, Deserialize)]
pub struct AgentSnapshot {
    /// The agent id.
    pub id: String,
    /// Whether the config marks it enabled.
    pub enabled: bool,
    /// Whether the daemon is currently ticking it (not paused).
    pub running: bool,
    /// When it last ticked, if ever.
    pub last_tick: Option<String>,
    /// Sessions currently running for it.
    pub active_sessions: u32,
    /// Short summaries of the most recent job outcomes.
    pub last_results: Vec<String>,
}

/// The live connection state the TUI shows for the daemon.
#[derive(Debug, Clone, Default)]
pub enum AgentsStatus {
    /// No snapshot yet — the first poll is in flight.
    #[default]
    Connecting,
    /// The daemon answered with a snapshot.
    Connected(DaemonSnapshot),
    /// The daemon could not be reached (with the reason).
    Offline(String),
}

/// The newest `.log` file for `agent_id` under the daemon's log dir, if any.
#[must_use]
pub fn latest_log(agent_id: &str) -> Option<PathBuf> {
    let dir = state_dir()?.join("logs").join(agent_id);
    newest_log_in(&dir)
}

/// The most recently modified `.log` in `dir`, or `None` if there are none.
fn newest_log_in(dir: &std::path::Path) -> Option<PathBuf> {
    let mut newest: Option<(std::time::SystemTime, PathBuf)> = None;
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if path.extension() != Some(std::ffi::OsStr::new("log")) {
            continue;
        }
        let Ok(modified) = entry.metadata().and_then(|meta| meta.modified()) else {
            continue;
        };
        let replace = match &newest {
            Some((newest_time, _)) => modified > *newest_time,
            None => true,
        };
        if replace {
            newest = Some((modified, path));
        }
    }
    newest.map(|(_, path)| path)
}

#[cfg(test)]
#[path = "tests/agentd.rs"]
mod tests;
