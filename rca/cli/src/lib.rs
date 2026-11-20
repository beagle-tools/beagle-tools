//! `beagle` — a terminal UI for root-cause analysis (RCA) workspaces.
//!
//! Each debugged system gets a workspace under `rcas/<id>/` containing a TOML
//! manifest plus markdown sections and ASCII diagrams. External tools
//! (typically Claude) write those files; this crate renders them as a tabbed
//! TUI so a human can understand what broke, why it broke, and how to fix it.
//!
//! Module layering (dependencies point strictly downward):
//!
//! ```text
//! ui ──▶ store ──▶ model
//! ```
//!
//! Markdown and ANSI diagram rendering come from `beagle_tui` (`markdown`,
//! `ansi`).
//!
//! `config` (the user config file) and `update` (self-update against GitHub
//! releases) sit beside `ui` at the top of the stack; both depend only on
//! `error`.
//!
//! The command line is part of the library too: [`run`] is the entry point
//! the unified `beagle` command links as `beagle rca` (the tool contract:
//! [`ABOUT`] + `run(args) -> ExitCode`), and [`run_standalone`] backs the
//! standalone rca binary (`src/main.rs`). Parsing (`cli`) is pure; `run`
//! and `commands` do the I/O.

pub mod agentd;
pub mod banner;
mod cli;
mod commands;
pub mod config;
pub mod error;
pub mod fuzzy;
pub mod handoff;
pub mod links;
pub mod model;
pub mod notify;
pub mod prs;
mod run;
pub mod similar;
pub mod skill;
pub mod store;
pub mod tickets;
pub mod tz;
pub mod ui;
pub mod update;

pub use cli::json::list as list_json;
pub use error::Error;
pub use run::{effective_root, run, run_standalone, ABOUT};
