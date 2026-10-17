//! Thin wrappers around the `tmux` binary — the "container runtime".
//!
//! Session lookup and teardown live in [`agent_core::tmux`] so every binary
//! targets sessions the same (exact-name) way; this module adds only what
//! the daemon alone does: starting them.

use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};
use beagle_utils::shell::quote;

/// Start a detached session named `name` in `cwd`, executing `script` via
/// `sh -c` regardless of the user's default tmux shell.
pub fn new_session(name: &str, cwd: &Path, script: &str) -> Result<()> {
    let command = format!("sh -c {}", quote(script));
    // Deliberately the daemon's own PATH (set by the launchd plist), not
    // `agent_core::tmux::command()`'s augmented one: the first new-session
    // starts the tmux server, and every run inherits that server's PATH.
    let output = Command::new("tmux")
        .args(["new-session", "-d", "-s", name, "-c"])
        .arg(cwd)
        .arg(command)
        .output()
        .context("failed to execute tmux")?;
    if !output.status.success() {
        bail!(
            "tmux new-session failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

/// Whether a session with exactly this name exists.
pub fn has_session(name: &str) -> Result<bool> {
    Ok(agent_core::tmux::has_session(name)?)
}

/// Kill the session if it exists; missing sessions are not an error.
pub fn kill_session(name: &str) -> Result<()> {
    Ok(agent_core::tmux::kill_session(name)?)
}
