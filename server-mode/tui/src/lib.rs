//! `beagle server-mode` — keep a Mac awake as a server while the screen
//! locks.
//!
//! `on` holds a `caffeinate -i -s` power assertion (no idle sleep, and no
//! system sleep on AC power; the display still sleeps and the screen still
//! locks), optionally ends it after `-t <hours>`, and optionally runs a
//! startup file in the background in its own process group. `off` stops all
//! of it. State is one value per file under
//! `$XDG_STATE_HOME/beagle/server-mode/` (default
//! `~/.local/state/beagle/server-mode/`); `status`, `off` and `log` also
//! understand a session the old standalone script started in
//! `~/.local/state/server-mode/`.
//!
//! ```text
//! run ──▶ cli (parse) ──▶ session (on/off/status/log/lock)
//!                              ├──▶ state (one value per file)
//!                              └──▶ procs (spawn detached, signal, stop)
//! ```

mod cli;
#[cfg(unix)]
mod procs;
#[cfg(unix)]
mod session;
#[cfg(unix)]
mod state;

#[cfg(all(test, unix))]
mod tests;

use std::ffi::OsString;
use std::process::ExitCode;

/// One line for `beagle help`.
pub const ABOUT: &str = "keep this Mac awake as a server while the screen locks";

/// `beagle server-mode --help`.
const HELP: &str = "\
beagle server-mode — keep this Mac awake as a server while the screen locks.

  beagle server-mode [on]                       stay awake until `beagle server-mode off`
  beagle server-mode on -t 8                    stay awake for 8 hours (decimals ok)
  beagle server-mode on --start-up -f run.sh    also run run.sh in the background
  beagle server-mode on --no-lock               stay awake without locking now
  beagle server-mode off                        let it sleep again, stop the file
  beagle server-mode status                     on/off, time left, startup file
  beagle server-mode log                        tail the startup file's output
  beagle server-mode lock                       lock the screen now

Awake is `caffeinate -i -s`: no idle sleep (and no system sleep on AC), but
the display still sleeps and the screen locks; `on` locks it right away.
Closing the lid still sleeps the Mac unless it is on power with an external
display. The startup file runs in its own process group (executable files
as-is, anything else with bash); `off`, or the timer running out, stops it
and everything it spawned. macOS only.
";

/// Prefix for every error line.
const PROGRAM: &str = "beagle server-mode";

/// Runs `beagle server-mode` with the arguments after `server-mode`.
pub fn run(args: Vec<OsString>) -> ExitCode {
    let words: Vec<String> = args
        .into_iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    let cmd = match cli::parse(&words) {
        Ok(cmd) => cmd,
        Err(e) => {
            eprintln!("{PROGRAM}: {e}");
            return ExitCode::from(2);
        }
    };
    if cmd == cli::Cmd::Help {
        print!("{HELP}");
        return ExitCode::SUCCESS;
    }
    execute(cmd)
}

#[cfg(target_os = "macos")]
fn execute(cmd: cli::Cmd) -> ExitCode {
    let result = session::Ctx::system().and_then(|ctx| {
        let mut out = std::io::stdout().lock();
        ctx.exec(cmd, &mut out)
    });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{PROGRAM}: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn execute(_cmd: cli::Cmd) -> ExitCode {
    eprintln!("{PROGRAM}: macOS only (it keeps a Mac awake with caffeinate)");
    ExitCode::FAILURE
}
