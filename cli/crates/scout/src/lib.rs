//! `beagle scout`: a live view of what an agent is working on.
//!
//! A **project** is one overarching goal; its **tasks** are the small,
//! ordered steps that get there. Both are plain Markdown files with TOML
//! frontmatter on disk (see [`store`]), written by agents through the
//! `beagle scout` CLI and the `/scout` skill, and rendered live by the TUI
//! ([`ui`]).
//!
//! The crate follows the `beagle` tool contract: [`ABOUT`] and [`run`].

mod app;
mod cli;

pub mod autopause;
pub mod config;
pub mod editor;
pub mod error;
pub mod frontmatter;
pub mod hooks;
pub mod markdown;
pub mod model;
pub mod notify;
pub mod notify_app;
pub mod picker;
pub mod session;
pub mod skill;
pub mod store;
pub mod transcript;
pub mod transcript_view;
pub mod ui;
pub mod watch;

pub use error::{Error, Result};

/// One line for `beagle help`.
pub const ABOUT: &str = "watch agent work as projects of small, ordered tasks";

/// Runs `beagle scout` with the arguments after `scout` (`--help` prints
/// the usage). Returns 2 on a usage error, 1 when the command fails.
pub fn run(args: Vec<std::ffi::OsString>) -> std::process::ExitCode {
    app::main(args)
}
