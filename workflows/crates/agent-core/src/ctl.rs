//! The name users type to reach the control CLI, for hints and usage lines.
//!
//! The same code runs as the standalone `agentctl` and as `beagle agent`
//! inside the unified `beagle` command. A hint like "run `agentctl apply`"
//! must name the command the user actually has, so the entry point records
//! it once at startup ([`set_name`]) and every hint reads [`name`].

use std::sync::OnceLock;

/// The standalone binary's name, used when no host has called [`set_name`].
pub const STANDALONE: &str = "agentctl";

static NAME: OnceLock<&'static str> = OnceLock::new();

/// Record the control command's name (e.g. `beagle agent`). The first call
/// wins; later calls are ignored, so a host and a tool can't disagree
/// mid-run.
pub fn set_name(name: &'static str) {
    let _ = NAME.set(name);
}

/// The control command's name: what [`set_name`] recorded, else
/// [`STANDALONE`].
pub fn name() -> &'static str {
    NAME.get().copied().unwrap_or(STANDALONE)
}
