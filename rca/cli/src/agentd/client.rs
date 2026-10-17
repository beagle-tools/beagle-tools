//! Control-socket requests to a running daemon (newline-JSON, one request
//! and one response line per connection) and the TUI's background status
//! poller.

use std::fmt::Write as _;
use std::io::{BufRead, BufReader, Write as _};
use std::os::unix::net::UnixStream;
use std::sync::mpsc::Sender;
use std::thread;
use std::time::Duration;

use super::{socket_path, AgentsStatus, DaemonSnapshot};

/// How often the TUI status poller re-queries the daemon.
const POLL_INTERVAL: Duration = Duration::from_secs(2);

/// Queries the running daemon and formats its status for `beagle agent status`.
///
/// # Errors
/// Returns a message if the daemon is not reachable or its response is
/// malformed.
pub fn status() -> Result<String, String> {
    Ok(format_snapshot(&fetch_status()?))
}

/// Connects to the control socket, sends `get-status`, and decodes the reply.
///
/// # Errors
/// Returns a message if the daemon is not reachable or its response cannot be
/// parsed.
pub fn fetch_status() -> Result<DaemonSnapshot, String> {
    parse_status(&send_line(&serde_json::json!({ "type": "get-status" }))?)
}

/// Resumes ticking `id`.
///
/// # Errors
/// Returns a message if the daemon is unreachable or rejects the request.
pub fn start_agent(id: &str) -> Result<(), String> {
    request_ok(&serde_json::json!({ "type": "start-agent", "id": id }))
}

/// Pauses `id`.
///
/// # Errors
/// Returns a message if the daemon is unreachable or rejects the request.
pub fn stop_agent(id: &str) -> Result<(), String> {
    request_ok(&serde_json::json!({ "type": "stop-agent", "id": id }))
}

/// Asks the daemon to re-read its config.
///
/// # Errors
/// Returns a message if the daemon is unreachable or rejects the request.
pub fn reload_config() -> Result<(), String> {
    request_ok(&serde_json::json!({ "type": "reload-config" }))
}

/// Spawns a background thread that polls the daemon every couple of seconds and
/// reports the connection state over `tx`. Exits when the receiver is dropped.
pub fn spawn_status_poller(tx: Sender<AgentsStatus>) {
    thread::spawn(move || loop {
        let status = match fetch_status() {
            Ok(snapshot) => AgentsStatus::Connected(snapshot),
            Err(reason) => AgentsStatus::Offline(reason),
        };
        if tx.send(status).is_err() {
            return; // the UI is gone
        }
        thread::sleep(POLL_INTERVAL);
    });
}

/// Sends one request and requires an `ok` response.
fn request_ok(payload: &serde_json::Value) -> Result<(), String> {
    let line = send_line(payload)?;
    let value: serde_json::Value =
        serde_json::from_str(line.trim()).map_err(|err| format!("bad daemon response: {err}"))?;
    match value.get("type").and_then(serde_json::Value::as_str) {
        Some("ok") => Ok(()),
        Some("error") => Err(error_message(&value)),
        _ => Err("unexpected daemon response".to_string()),
    }
}

/// Connects to the control socket, writes one JSON request line, and returns the
/// single response line.
fn send_line(payload: &serde_json::Value) -> Result<String, String> {
    let path = socket_path();
    let stream = UnixStream::connect(&path).map_err(|err| {
        format!(
            "daemon not reachable at {} ({err}); is it running? try `beagle agent start`",
            path.display()
        )
    })?;
    let mut writer = stream
        .try_clone()
        .map_err(|err| format!("socket error: {err}"))?;
    let mut reader = BufReader::new(stream);
    let mut request = payload.to_string();
    request.push('\n');
    writer
        .write_all(request.as_bytes())
        .map_err(|err| format!("socket write: {err}"))?;
    writer
        .flush()
        .map_err(|err| format!("socket flush: {err}"))?;
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .map_err(|err| format!("socket read: {err}"))?;
    Ok(line)
}

/// Decodes a `get-status` response line into a [`DaemonSnapshot`], turning an
/// `error` response into an `Err`.
pub(super) fn parse_status(line: &str) -> Result<DaemonSnapshot, String> {
    let value: serde_json::Value =
        serde_json::from_str(line.trim()).map_err(|err| format!("bad daemon response: {err}"))?;
    match value.get("type").and_then(serde_json::Value::as_str) {
        Some("status") => {
            let status = value
                .get("status")
                .ok_or("unexpected daemon response (no status)")?;
            serde_json::from_value(status.clone())
                .map_err(|err| format!("bad status payload: {err}"))
        }
        Some("error") => Err(error_message(&value)),
        _ => Err("unexpected daemon response".to_string()),
    }
}

/// The `message` of an `error` response, or a generic fallback when the
/// daemon sent none.
fn error_message(value: &serde_json::Value) -> String {
    value
        .get("message")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("daemon error")
        .to_string()
}

/// Formats a snapshot into a human-readable summary for the CLI.
pub(super) fn format_snapshot(snapshot: &DaemonSnapshot) -> String {
    let mut out = format!("beagle-agentd {}", snapshot.version);
    if snapshot.agents.is_empty() {
        out.push_str("\n  (no agents)");
        return out;
    }
    for agent in &snapshot.agents {
        let state = if agent.running { "running" } else { "paused" };
        let _ = write!(out, "\n  {}: {state}", agent.id);
        if let Some(tick) = &agent.last_tick {
            let _ = write!(out, " (last tick {tick})");
        }
    }
    out
}
