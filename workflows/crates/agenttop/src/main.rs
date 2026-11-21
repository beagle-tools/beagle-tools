//! The standalone `agenttop` binary: [`agenttop::run_as`] under its own
//! name. Users of the unified `beagle` command open the same TUI as
//! `beagle agent` (or `beagle agent top`).
//!
//! Deprecated: kept for one release so scripts and services set up before
//! the switch keep working, then removed.

use std::process::ExitCode;

fn main() -> ExitCode {
    // Only at a terminal: scripts (and runs started through `$AGENTD_CTL`)
    // keep a clean stderr.
    if std::io::IsTerminal::is_terminal(&std::io::stderr()) {
        eprintln!("note: `agenttop` is deprecated and goes away in the next release; use `beagle agent top`");
    }
    agenttop::run_as(agenttop::STANDALONE, std::env::args_os().skip(1).collect())
}
