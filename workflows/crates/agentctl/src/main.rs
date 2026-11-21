//! The standalone `agentctl` binary: [`agentctl::run_as`] under its own
//! name. Users of the unified `beagle` command run the same code as
//! `beagle agent …`.

use std::process::ExitCode;

fn main() -> ExitCode {
    agentctl::run_as(
        agent_core::ctl::STANDALONE,
        std::env::args_os().skip(1).collect(),
    )
}
