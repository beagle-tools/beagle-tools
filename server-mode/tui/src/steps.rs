//! Running the `[[before]]` / `[[after]]` steps: `sh -c <run>` bounded by
//! the step's timeout, then `wait` retried until it exits 0. Each step's
//! output is appended to the session log (`beagle server-mode log`).

use std::io::Write as _;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use beagle_utils::process::{command, output_with_timeout};

use crate::config::Step;

/// Pause between two tries of a step's `wait`.
const WAIT_EVERY: Duration = Duration::from_millis(500);

/// Bound on one try of `wait` (the whole wait is the step's timeout).
const WAIT_TRY: Duration = Duration::from_secs(10);

/// Runs `script` with `sh`; `Ok(())` when it exits 0 within `timeout`,
/// else what went wrong. Its output is appended to `log`.
fn sh(sh: &Path, script: &str, timeout: Duration, log: &Path) -> Result<(), String> {
    let mut cmd = command(sh);
    cmd.args(["-c", script]);
    let out = output_with_timeout(&mut cmd, timeout).map_err(|e| e.to_string())?;
    append(log, &out.stdout);
    append(log, &out.stderr);
    if out.status.success() {
        return Ok(());
    }
    let last = String::from_utf8_lossy(&out.stderr)
        .lines()
        .rev()
        .chain(String::from_utf8_lossy(&out.stdout).lines().rev())
        .map(str::trim)
        .find(|l| !l.is_empty())
        .map(str::to_owned);
    Err(match (out.status.code(), last) {
        (Some(code), Some(line)) => format!("exited {code}: {line}"),
        (Some(code), None) => format!("exited {code}"),
        (None, _) => "killed by a signal".to_owned(),
    })
}

/// Appends `bytes` to the log; a log that can't be written is not worth
/// failing a step over.
pub(crate) fn append(log: &Path, bytes: &[u8]) {
    if bytes.is_empty() {
        return;
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log)
    {
        let _ = f.write_all(bytes);
    }
}

/// Runs one step: `run`, then `wait` until it succeeds or the step's
/// timeout passes. The error says which part failed and how.
pub(crate) fn run(shell: &Path, step: &Step, log: &Path) -> Result<(), String> {
    let limit = Duration::from_secs(step.timeout_secs());
    append(log, format!("== {}: {}\n", step.name, step.run).as_bytes());
    sh(shell, &step.run, limit, log).map_err(|e| format!("`{}` {e}", step.run))?;
    let Some(wait) = &step.wait else {
        return Ok(());
    };
    let end = Instant::now() + limit;
    loop {
        let left = end.saturating_duration_since(Instant::now());
        let last = sh(
            shell,
            wait,
            left.min(WAIT_TRY).max(Duration::from_secs(1)),
            log,
        );
        match last {
            Ok(()) => return Ok(()),
            Err(e) if Instant::now() >= end => {
                return Err(format!(
                    "`{wait}` still failing after {}s ({e})",
                    limit.as_secs()
                ));
            }
            Err(_) => thread::sleep(WAIT_EVERY),
        }
    }
}
