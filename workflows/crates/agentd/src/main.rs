//! The standalone `agentd` binary: [`agentd::run_as`] under its own name.
//! Users of the unified `beagle` command run the same daemon as
//! `beagle agent daemon run`.
//!
//! Deprecated: kept for one release so scripts and services set up before
//! the switch keep working, then removed.

use std::process::ExitCode;

fn main() -> ExitCode {
    // Once, into the service's log: whoever reads it learns the fix.
    eprintln!("note: `agentd` is deprecated and goes away in the next release; use `beagle agent daemon run` (reinstall the service with `beagle agent daemon install`)");
    agentd::run_as(agentd::STANDALONE, std::env::args_os().skip(1).collect())
}
