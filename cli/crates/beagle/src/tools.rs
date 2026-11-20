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
        name: "rca",
        about: beagle_rca::ABOUT,
        run: beagle_rca::run,
    },
    Tool {
        name: "recap",
        about: recap::ABOUT,
        run: recap::run,
    },
    Tool {
        name: "scout",
        about: beagle_scout::ABOUT,
        run: beagle_scout::run,
    },
    Tool {
        name: "serve",
        about: beagle_serve::ABOUT,
        run: beagle_serve::run,
    },
    Tool {
        name: "standup",
        about: standup::ABOUT,
        run: standup::run,
    },
    Tool {
        name: "tandem",
        about: tandem_cli::ABOUT,
        run: tandem,
    },
];

/// `beagle tandem`: tandem-cli under the name the user typed, so its help
/// and errors say `beagle tandem`, its `update` defers to ours, and
/// `--version` prints beagle's version.
fn tandem(args: Vec<OsString>) -> ExitCode {
    tandem_cli::run_hosted("beagle tandem", env!("CARGO_PKG_VERSION"), args)
}

/// The tool called `name`, if there is one.
pub fn find(name: &str) -> Option<&'static Tool> {
    TOOLS.iter().find(|t| t.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Names `beagle` itself answers before looking up a tool (`main.rs`).
    const BUILTINS: &[&str] = &["help", "version", "update", "doctor"];

    #[test]
    fn tools_are_unique_sorted_and_never_shadow_a_builtin() {
        let names: Vec<&str> = TOOLS.iter().map(|t| t.name).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(names, sorted, "TOOLS stays alphabetical, no duplicates");
        for name in names {
            assert!(!BUILTINS.contains(&name), "`{name}` is a builtin");
        }
    }

    #[test]
    fn every_tool_is_findable() {
        for name in ["rca", "recap", "scout", "serve", "standup", "tandem"] {
            assert!(find(name).is_some(), "{name}");
        }
        assert!(find("update").is_none());
    }
}
