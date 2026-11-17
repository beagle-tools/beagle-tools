//! The tool registry: every `beagle <tool>` the binary knows.
//!
//! A tool is a library crate that exposes `pub fn run(args: Vec<OsString>)
//! -> ExitCode` taking the arguments AFTER its own name, and a one-line
//! `ABOUT`. Adding one is a dependency in this crate's `Cargo.toml` plus
//! one entry in [`TOOLS`]; nothing else here changes.

use std::ffi::OsString;
use std::process::ExitCode;

/// One subcommand of `beagle`.
#[derive(Debug, Clone, Copy)]
pub struct Tool {
    /// What the user types: `beagle <name> …`.
    pub name: &'static str,
    /// One line for `beagle help`.
    pub about: &'static str,
    /// Runs the tool with the arguments after its name.
    pub run: fn(Vec<OsString>) -> ExitCode,
}

/// Every registered tool, in the order `beagle help` lists them.
pub const TOOLS: &[Tool] = &[
    Tool {
        name: "scout",
        about: beagle_scout::ABOUT,
        run: beagle_scout::run,
    },
    Tool {
        name: "standup",
        about: standup::ABOUT,
        run: standup::run,
    },
];

/// The tool called `name`, if there is one.
pub fn find(name: &str) -> Option<&'static Tool> {
    TOOLS.iter().find(|t| t.name == name)
}
