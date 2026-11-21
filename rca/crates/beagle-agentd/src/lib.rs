//! `beagle-agentd` — the always-on daemon that hosts beagle's agent engine.
//!
//! A library entry point ([`run_as`]): the unified `beagle` command runs it
//! as `beagle rca agentd run` (that is what `beagle rca agent install`
//! puts in the service unit), and `main.rs` is the standalone
//! `beagle-agentd`. `once` runs a single tick per agent and exits (handy for
//! a manual end-to-end); `run`, or no argument, loops.

mod daemon;

use std::ffi::OsString;
use std::process::ExitCode;

/// The standalone daemon's program name.
pub const STANDALONE: &str = "beagle-agentd";

/// What the arguments ask for.
#[derive(Debug, PartialEq, Eq)]
enum Action {
    /// Tick forever and serve the control socket.
    Run,
    /// One tick per agent, then exit.
    Once,
    Version,
    Help,
}

/// Runs the daemon as `program` (`beagle-agentd`, or `beagle rca agentd`)
/// with the arguments after it. Returns only for `once`, `version`, `help`,
/// a usage error, or a fatal setup error.
pub fn run_as(program: &str, args: Vec<OsString>) -> ExitCode {
    let action = match parse(args) {
        Ok(action) => action,
        Err(bad) => {
            eprintln!(
                "{program}: unknown argument `{bad}` (use `run`, `once`, `version` or `help`)"
            );
            return ExitCode::from(2);
        }
    };
    let once = match action {
        Action::Version => {
            println!("{program} {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        Action::Help => {
            println!("{}", usage(program));
            return ExitCode::SUCCESS;
        }
        Action::Run => false,
        Action::Once => true,
    };
    match daemon::run(once) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("beagle-agentd: {err}");
            ExitCode::FAILURE
        }
    }
}

fn parse(args: Vec<OsString>) -> Result<Action, String> {
    let mut args = args.into_iter();
    let first = args.next();
    let action = match first.as_ref().map(|a| a.to_string_lossy()).as_deref() {
        None | Some("run") => Action::Run,
        Some("once") => Action::Once,
        Some("version" | "--version" | "-V") => Action::Version,
        Some("help" | "--help" | "-h") => Action::Help,
        Some(other) => return Err(other.to_owned()),
    };
    match args.next() {
        Some(extra) => Err(extra.to_string_lossy().into_owned()),
        None => Ok(action),
    }
}

fn usage(program: &str) -> String {
    format!(
        "{program} — the daemon that implements RCAs handed to agents\n\n\
         Usage: {program} [run | once | version | help]\n\n  \
         run      tick every agent forever and serve the control socket (the default)\n  \
         once     one tick per agent, then exit\n  \
         version  print the version\n\n\
         Install it as a login service with `beagle rca agent install`."
    )
}

#[cfg(test)]
mod tests {
    use super::{parse, Action};
    use std::ffi::OsString;

    fn args(list: &[&str]) -> Vec<OsString> {
        list.iter().map(OsString::from).collect()
    }

    #[test]
    fn arguments_pick_the_action() {
        assert_eq!(parse(args(&[])), Ok(Action::Run));
        assert_eq!(parse(args(&["run"])), Ok(Action::Run));
        assert_eq!(parse(args(&["once"])), Ok(Action::Once));
        assert_eq!(parse(args(&["--version"])), Ok(Action::Version));
        assert_eq!(parse(args(&["--help"])), Ok(Action::Help));
    }

    #[test]
    fn anything_else_is_a_usage_error() {
        assert_eq!(parse(args(&["start"])), Err("start".to_owned()));
        assert_eq!(parse(args(&["run", "now"])), Err("now".to_owned()));
    }
}
