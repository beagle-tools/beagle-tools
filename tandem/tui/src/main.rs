//! `tandem-cli`: the standalone binary. All logic lives in the library so
//! the unified `beagle` command can link the same thing as `beagle tandem`.

use std::process::ExitCode;

fn main() -> ExitCode {
    tandem_cli::run(std::env::args_os().skip(1).collect())
}
