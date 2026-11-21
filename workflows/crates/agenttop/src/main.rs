//! The standalone `agenttop` binary: [`agenttop::run_as`] under its own
//! name. Users of the unified `beagle` command open the same TUI as
//! `beagle agent` (or `beagle agent top`).

use std::process::ExitCode;

fn main() -> ExitCode {
    agenttop::run_as(agenttop::STANDALONE, std::env::args_os().skip(1).collect())
}
