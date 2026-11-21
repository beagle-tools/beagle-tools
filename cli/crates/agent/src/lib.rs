//! `beagle agent` — the agentd stack (`workflows/`) under the unified
//! command.
//!
//! The commands are agentctl's, run under this name ([`PROGRAM`]), so help,
//! usage and hints all say `beagle agent`. agentctl also owns `top` (the
//! dashboard) and `daemon run` (the daemon itself), so one binary covers
//! the CLI, the TUI and the launchd service. The only thing added here is
//! the default: a bare `beagle agent` opens the dashboard, where the
//! standalone `agentctl` prints its help.

use std::ffi::OsString;
use std::process::ExitCode;

/// One line for `beagle help`.
pub const ABOUT: &str = agentctl::ABOUT;

/// What the user types to reach this tool. The launchd unit that
/// `beagle agent daemon install` writes runs `<beagle> agent daemon run`.
pub const PROGRAM: &str = "beagle agent";

/// Runs `beagle agent` with the arguments after `agent`.
pub fn run(args: Vec<OsString>) -> ExitCode {
    agentctl::run_as(PROGRAM, route(args))
}

/// No arguments means the dashboard; anything else is agentctl's to parse.
fn route(args: Vec<OsString>) -> Vec<OsString> {
    if args.is_empty() {
        vec![OsString::from("top")]
    } else {
        args
    }
}

#[cfg(test)]
mod tests {
    use super::route;
    use std::ffi::OsString;

    fn args(list: &[&str]) -> Vec<OsString> {
        list.iter().map(OsString::from).collect()
    }

    #[test]
    fn bare_opens_the_dashboard() {
        assert_eq!(route(Vec::new()), args(&["top"]));
    }

    #[test]
    fn everything_else_passes_through_unchanged() {
        for case in [
            &["top", "--help"][..],
            &["daemon", "run", "--once"],
            &["daemon", "install", "--print"],
            &["-j", "hello", "-p", "name=you"],
            &["get", "agents"],
            &["no-such-command"],
        ] {
            assert_eq!(route(args(case)), args(case), "{case:?}");
        }
    }
}
