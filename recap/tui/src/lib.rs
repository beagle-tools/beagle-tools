//! `beagle recap` — a terminal UI for PR recaps.
//!
//! A recap is a directory under the store root's `recaps/`, one per pull
//! request (`recaps/<repo>-<number>/`). It holds the PR's metadata
//! (`recap.toml`), its diff as pulled from GitHub (`diff.patch`), a
//! plain-language write-up (`summary.md`) and optional ASCII `diagrams/`.
//! Agents (or you) write the summary; this crate renders it next to the diff
//! so you can get up to speed on a PR in a couple of minutes.
//!
//! Module layering (dependencies point strictly downward):
//!
//! ```text
//! run ──▶ cli
//!  └──▶ ui ──▶ store ──▶ model
//!        │       └──▶ gh
//!        ├──▶ poller (background `gh` status thread)
//!        ├──▶ comments (threads, read marks, agent reviews)
//!        ├──▶ diff
//!        └──▶ beagle_tui (markdown, ansi, wrap, DocView, widgets)
//! ```

pub mod actions;
pub mod cli;
pub mod comments;
pub mod config;
pub mod diff;
pub mod error;
pub mod gh;
pub mod model;
pub mod poller;
mod run;
pub mod store;
pub mod ui;

pub use beagle_tui::{ansi, clipboard, links, markdown, wrap};
pub use error::Error;

use std::ffi::OsString;
use std::process::ExitCode;

/// One line for `beagle help`.
pub const ABOUT: &str =
    "PR recaps: the diff plus a plain-language write-up of what it fixes and why";

/// The agent guide, printed by `beagle recap guide` so any agent, in any
/// directory, can learn the CLI, the on-disk format and the workflow.
pub const AGENT_GUIDE: &str = include_str!("../docs/AGENT_GUIDE.md");

/// Runs `beagle recap` with the arguments after `recap`.
pub fn run(args: Vec<OsString>) -> ExitCode {
    run::main(args)
}
