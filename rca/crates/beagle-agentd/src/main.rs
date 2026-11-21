//! The standalone `beagle-agentd` binary: [`beagle_agentd::run_as`] under
//! its own name. Users of the unified `beagle` command run the same daemon
//! as `beagle rca agentd run`.
//!
//! Deprecated: kept for one release so scripts and services set up before
//! the switch keep working, then removed.

use std::process::ExitCode;

fn main() -> ExitCode {
    // Once, into the service's log: whoever reads it learns the fix.
    eprintln!("note: `beagle-agentd` is deprecated and goes away in the next release; use `beagle rca agentd run` (reinstall the service with `beagle rca agent install`)");
    beagle_agentd::run_as(
        beagle_agentd::STANDALONE,
        std::env::args_os().skip(1).collect(),
    )
}
