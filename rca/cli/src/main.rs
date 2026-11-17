//! The standalone rca `beagle` binary: a thin wrapper over the library, so
//! it behaves exactly like `beagle rca` in the unified `beagle` command
//! (plus its own self-update, until this binary is retired).

use std::process::ExitCode;

fn main() -> ExitCode {
    beagle::run_standalone(std::env::args_os().skip(1).collect())
}
