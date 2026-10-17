//! Entry point for `beagle`: build the context, run, then exit or exec.

use std::process::ExitCode;

use beagle_cli::{dispatch, run, Context, Outcome};

fn main() -> ExitCode {
    let ctx = match Context::from_env() {
        Ok(ctx) => ctx,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(e.exit_code());
        }
    };
    let mut stdout = std::io::stdout();
    match run(std::env::args_os().skip(1).collect(), &ctx, &mut stdout) {
        Ok(Outcome::Exit(code)) => ExitCode::from(code),
        Ok(Outcome::Exec(plan)) => {
            let e = dispatch::exec(plan.command, &plan.resolved);
            eprintln!("error: {e}");
            ExitCode::from(e.exit_code())
        }
        Err(e) => {
            eprintln!("error: {e}");
            if e.exit_code() == 2 && matches!(e, beagle_cli::Error::Usage(_)) {
                eprintln!("\nrun `beagle --help` for usage");
            }
            ExitCode::from(e.exit_code())
        }
    }
}
