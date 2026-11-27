//! `beagle server-mode` — get a Mac ready to serve the Beagle phone app,
//! and keep it awake while the screen locks.
//!
//! `on` runs the user's `[[before]]` steps from `server-mode.toml` (e.g.
//! connect a VPN), starts `beagle serve` (its launchd agent) and waits for
//! its health check, holds a `caffeinate -i -s` power assertion (no idle
//! sleep, and no system sleep on AC power; the display still sleeps and the
//! screen still locks), optionally ends it after `-t <hours>`, optionally
//! runs a startup file in its own process group, checks the certificate,
//! agentd and exposure, locks the screen and prints one ready summary.
//! `off` (or the timer) stops all of it, stops `beagle serve`, and runs the
//! `--on-stop` file and the `[[after]]` steps. State is one value per file
//! under `$XDG_STATE_HOME/beagle/server-mode/` (default
//! `~/.local/state/beagle/server-mode/`); `status`, `off` and `log` also
//! understand a session the old standalone script started in
//! `~/.local/state/server-mode/`.
//!
//! ```text
//! run ──▶ cli (parse) ──▶ session (on/off) · report (status/log/lock)
//!                              ├──▶ config (server-mode.toml) · steps (sh -c)
//!                              ├──▶ host (power, lock, serve, agentd; faked in tests)
//!                              ├──▶ state (one value per file)
//!                              └──▶ procs (spawn detached, signal, stop)
//! ```

// Only macOS runs the commands; elsewhere they still build (and their
// tests run) but nothing calls them.
#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

mod cli;
#[cfg(unix)]
mod config;
#[cfg(unix)]
mod host;
#[cfg(unix)]
mod procs;
#[cfg(unix)]
mod report;
#[cfg(unix)]
mod session;
#[cfg(unix)]
mod state;
#[cfg(unix)]
mod steps;

#[cfg(all(test, unix))]
mod tests;

use std::ffi::OsString;
use std::process::ExitCode;

/// One line for `beagle help`.
pub const ABOUT: &str = "get this Mac ready to serve the phone app, awake and locked";

/// `beagle server-mode --help`.
const HELP: &str = "\
beagle server-mode — get this Mac ready to serve the Beagle phone app.

  beagle server-mode [on]                       ready + awake until `beagle server-mode off`
  beagle server-mode on -t 8                    for 8 hours (decimals ok)
  beagle server-mode on --start-up -f run.sh    also run run.sh in the background
  beagle server-mode on --on-stop stop.sh       run stop.sh when server mode ends
  beagle server-mode on --new-pairing           rotate the pairing token, show the QR code
  beagle server-mode on --no-serve              don't start or stop beagle serve
  beagle server-mode on --no-lock               don't lock the screen now
  beagle server-mode off                        let it sleep again, stop beagle serve
  beagle server-mode status                     on/off, time left, the checklist
  beagle server-mode log                        the steps' and startup file's output
  beagle server-mode lock                       lock the screen now

`on`, in order: the [[before]] steps in ~/.config/beagle/server-mode.toml
(each `sh -c`, must exit 0), start beagle serve and wait for its health
check, keep the Mac awake, check the certificate, agentd and exposure,
lock the screen, and print \"Ready — open Beagle on your phone\" (or what's
wrong). `off`, or the timer running out, stops beagle serve, the startup
file and everything it spawned, then runs --on-stop and the [[after]] steps.

Awake is `caffeinate -i -s`: no idle sleep (and no system sleep on AC), but
the display still sleeps and the screen locks. Closing the lid still sleeps
the Mac unless it is on power with an external display. Flags override
server-mode.toml (keys: serve, new_pairing, lock, hours, start_up, on_stop,
[[before]], [[after]]; see server-mode/README.md). macOS only.
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
