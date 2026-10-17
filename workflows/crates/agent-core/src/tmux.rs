//! The one place every agentd binary talks to tmux about a run's session.
//!
//! Two rules every caller used to re-implement (and some got wrong):
//!
//! - **Exact names.** `tmux -t foo` prefix-matches, so killing run `ab`'s
//!   session could take down `abc`'s. Targets are always `=name`.
//! - **Findable binary.** A Finder-launched app inherits launchd's minimal
//!   PATH, where `/opt/homebrew/bin/tmux` does not exist. The binary is
//!   resolved on [`beagle_utils::process::augmented_path`] and the child gets
//!   that PATH too.

use std::path::PathBuf;
use std::process::Command;

use crate::CoreError;

/// A `tmux` command that works from a minimal-PATH parent (launchd, Finder).
///
/// Falls back to the bare name when tmux is nowhere on the augmented PATH,
/// so the spawn error names the program instead of an empty path.
pub fn command() -> Command {
    let program = beagle_utils::process::which("tmux").unwrap_or_else(|| PathBuf::from("tmux"));
    beagle_utils::process::command(program)
}

/// The `-t` target that matches the session named exactly `name` — never a
/// longer name that merely starts with it.
pub fn exact_target(name: &str) -> String {
    format!("={name}")
}

/// Whether a session with exactly this name exists.
pub fn has_session(name: &str) -> Result<bool, CoreError> {
    let output = command()
        .args(["has-session", "-t", &exact_target(name)])
        .output()
        .map_err(|e| CoreError::io("tmux", e))?;
    Ok(output.status.success())
}

/// Kill the session named exactly `name`.
///
/// A session that is already gone is not an error — the run ended either
/// way. Fails only when tmux cannot be spawned or the session survives.
pub fn kill_session(name: &str) -> Result<(), CoreError> {
    let output = command()
        .args(["kill-session", "-t", &exact_target(name)])
        .output()
        .map_err(|e| CoreError::io("tmux", e))?;
    if !output.status.success() && has_session(name)? {
        return Err(CoreError::Other(format!(
            "tmux kill-session failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(())
}

#[cfg(test)]
#[path = "tests/tmux.rs"]
mod tests;
