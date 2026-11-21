//! The standalone `agentctl` binary: [`agentctl::run_as`] under its own
//! name. Users of the unified `beagle` command run the same code as
//! `beagle agent …`.
//!
//! Deprecated: kept for one release so scripts and services set up before
//! the switch keep working, then removed.

use std::process::ExitCode;

fn main() -> ExitCode {
    // Only at a terminal: scripts (and runs started through `$AGENTD_CTL`)
    // keep a clean stderr.
    if std::io::IsTerminal::is_terminal(&std::io::stderr()) {
        eprintln!(
            "note: `agentctl` is deprecated and goes away in the next release; use `beagle agent`"
        );
    }
    agentctl::run_as(
        agent_core::ctl::STANDALONE,
        std::env::args_os().skip(1).collect(),
    )
}
