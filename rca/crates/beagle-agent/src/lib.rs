//! `beagle-agent` — the engine for beagle's native agent runtime.
//!
//! This crate holds the pieces that the [`beagle-agentd`] daemon assembles into
//! an always-on loop: agent configuration, durable job state, the RCA trigger
//! poller, git-worktree isolation, the headless `claude` runner, and the
//! control-socket protocol shared with the TUI and desktop clients.
//!
//! Subsystems land module by module.
//!
//! [`beagle-agentd`]: https://github.com/beagle-tools/beagle-tools/tree/main/rca/crates/beagle-agentd

pub mod config;
pub mod control;
pub mod orchestrator;
pub mod poller;
pub mod protocol;
pub mod runner;
pub mod store;
pub mod worktree;
