//! `beagle standup` — a terminal UI for standup reports.
//!
//! A report is a directory under the store root's `standups/`, named for its
//! date (`standups/2026-10-27/`). It holds a big-picture `overview.md`,
//! optional ASCII `diagrams/`, and one sub-directory per ticket under
//! `tickets/`. Agents (or you) write those files; this crate renders them so
//! a human can catch up on what happened since the last standup in a couple
//! of minutes.
//!
//! Module layering (dependencies point strictly downward):
//!
//! ```text
//! run ──▶ cli
//!  └──▶ ui ──▶ store ──▶ model
//!        └──▶ beagle_tui (markdown, ansi, wrap, DocView, widgets)
//! ```

pub mod cli;
pub mod config;
pub mod error;
pub mod model;
mod run;
pub mod store;
pub mod today;
pub mod ui;

pub use beagle_tui::{ansi, clipboard, links, markdown, wrap};
pub use error::Error;

use std::ffi::OsString;
use std::process::ExitCode;

/// One line for `beagle help`.
pub const ABOUT: &str = "standup reports: one overview, then a short write-up per ticket";

/// The agent guide, printed by `beagle standup guide` so any agent, in any
/// directory, can learn the CLI, the on-disk format and the workflow.
pub const AGENT_GUIDE: &str = include_str!("../docs/AGENT_GUIDE.md");

/// Runs `beagle standup` with the arguments after `standup`.
pub fn run(args: Vec<OsString>) -> ExitCode {
    run::main(args)
}
