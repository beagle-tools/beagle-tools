//! Running the external tools the base shells out to (`curl`, `tar`,
//! `shasum`), always with a timeout and a kill path.
//!
//! Shelling out keeps the binary free of HTTP/TLS/archive crates, matching
//! rca's updater. A stuck tool must never wedge `beagle`.

use std::ffi::OsStr;
use std::io::Read;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::error::{Error, Result};

/// Generous upper bound for a release download.
pub const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(300);
/// Upper bound for local tools (tar, shasum).
pub const LOCAL_TIMEOUT: Duration = Duration::from_secs(60);
/// How often a running tool is checked for exit; it only bounds how late a
/// timeout fires, so it can be coarse.
const POLL: Duration = Duration::from_millis(10);

/// Runs `tool args…` and returns its stdout. Fails with [`Error::Tool`] when
/// the tool is missing, exits non-zero (carrying its stderr), or runs longer
/// than `timeout` (it is killed).
pub fn run<S: AsRef<OsStr>>(tool: &'static str, args: &[S], timeout: Duration) -> Result<Vec<u8>> {
    let mut cmd = Command::new(tool);
    cmd.args(args);
    run_command(tool, cmd, timeout)
}

/// [`run`] for a prepared command; `label` names it in errors.
pub fn run_command(label: &'static str, mut cmd: Command, timeout: Duration) -> Result<Vec<u8>> {
    let tool = label;
    let fail = |message: String| Error::Tool { tool, message };
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            fail(if e.kind() == std::io::ErrorKind::NotFound {
                format!("`{tool}` not found on PATH")
            } else {
                e.to_string()
            })
        })?;
    // Drain both pipes on their own threads so a chatty tool can't block on
    // a full pipe while we wait for it.
    let stdout = child.stdout.take().map(drain);
    let stderr = child.stderr.take().map(drain);
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() > timeout => {
                let _ = child.kill(); // already exiting is fine
                let _ = child.wait();
                return Err(fail(format!("timed out after {}s", timeout.as_secs())));
            }
            Ok(None) => thread::sleep(POLL),
            Err(e) => return Err(fail(e.to_string())),
        }
    };
    let out = stdout.map(join).unwrap_or_default();
    let err = stderr.map(join).unwrap_or_default();
    if status.success() {
        Ok(out)
    } else {
        let message = String::from_utf8_lossy(&err).trim().to_owned();
        Err(fail(if message.is_empty() {
            format!("exited with {status}")
        } else {
            message
        }))
    }
}

fn drain(mut pipe: impl Read + Send + 'static) -> thread::JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = pipe.read_to_end(&mut buf); // a broken pipe just ends the read
        buf
    })
}

fn join(handle: thread::JoinHandle<Vec<u8>>) -> Vec<u8> {
    handle.join().unwrap_or_default()
}

/// `curl` arguments shared by every request: fail on HTTP errors, follow
/// redirects (release downloads redirect to a CDN), bounded connect time.
pub fn curl_args() -> Vec<String> {
    ["-fsSL", "--retry", "2", "--connect-timeout", "15"]
        .map(str::to_owned)
        .to_vec()
}

#[cfg(test)]
#[path = "tests/tool.rs"]
mod tests;
