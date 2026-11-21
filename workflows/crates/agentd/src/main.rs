//! The standalone `agentd` binary: [`agentd::run_as`] under its own name.
//! Users of the unified `beagle` command run the same daemon as
//! `beagle agent daemon run`.

use std::process::ExitCode;

fn main() -> ExitCode {
    agentd::run_as(agentd::STANDALONE, std::env::args_os().skip(1).collect())
}
