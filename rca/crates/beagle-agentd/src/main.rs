//! The standalone `beagle-agentd` binary: [`beagle_agentd::run_as`] under
//! its own name. Users of the unified `beagle` command run the same daemon
//! as `beagle rca agentd run`.

use std::process::ExitCode;

fn main() -> ExitCode {
    beagle_agentd::run_as(
        beagle_agentd::STANDALONE,
        std::env::args_os().skip(1).collect(),
    )
}
