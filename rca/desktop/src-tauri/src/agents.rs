//! The agents control surface: Tauri commands that proxy the `beagle-agentd`
//! control socket, plus a background emitter that streams live status to the
//! frontend.
//!
//! The desktop speaks the daemon's newline-JSON wire format directly — it
//! re-declares the small shape it reads rather than depending on the daemon
//! engine crate, so the app stays a thin renderer (no bundled `SQLite` etc.).
//! The DTOs below mirror `src/types.ts`.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter};

use crate::error::{Error, Result};

/// The event name the emitter pushes live status on.
const EVENT: &str = "agents-status";
/// How long to wait before reconnecting the subscribe stream.
const RECONNECT_DELAY: Duration = Duration::from_secs(2);

/// A snapshot of the whole daemon (mirrors the daemon's `DaemonStatus`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentsStatus {
    /// The daemon version.
    pub version: String,
    /// One entry per configured agent.
    pub agents: Vec<Agent>,
}

/// One agent's status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    /// The agent id.
    pub id: String,
    /// Whether the config marks it enabled.
    pub enabled: bool,
    /// Whether the daemon is currently ticking it (not paused).
    pub running: bool,
    /// When it last ticked, if ever.
    pub last_tick: Option<String>,
    /// Sessions currently running for it.
    pub active_sessions: u32,
    /// Short summaries of the most recent job outcomes.
    pub last_results: Vec<String>,
}

/// The payload pushed to the frontend on the `agents-status` event.
#[derive(Debug, Clone, Serialize)]
pub struct AgentsEvent {
    /// Whether the daemon is currently reachable.
    pub connected: bool,
    /// The latest snapshot, when connected.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<AgentsStatus>,
    /// Why the daemon is unreachable, when offline.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// One-shot status query.
///
/// # Errors
/// An [`Error`] if the daemon is unreachable or its reply is malformed.
#[tauri::command]
pub fn agents_status() -> Result<AgentsStatus> {
    status_from_value(&request(&serde_json::json!({ "type": "get-status" }))?)
}

/// Resumes ticking `id`.
///
/// # Errors
/// An [`Error`] if the daemon is unreachable or rejects the request.
#[tauri::command]
pub fn start_agent(id: &str) -> Result<()> {
    request_ok(&serde_json::json!({ "type": "start-agent", "id": id }))
}

/// Pauses `id`.
///
/// # Errors
/// An [`Error`] if the daemon is unreachable or rejects the request.
#[tauri::command]
pub fn stop_agent(id: &str) -> Result<()> {
    request_ok(&serde_json::json!({ "type": "stop-agent", "id": id }))
}

/// Asks the daemon to re-read its config.
///
/// # Errors
/// An [`Error`] if the daemon is unreachable or rejects the request.
#[tauri::command]
pub fn reload_agents_config() -> Result<()> {
    request_ok(&serde_json::json!({ "type": "reload-config" }))
}

/// Spawns the background emitter: it subscribes to the daemon and pushes an
/// `agents-status` event per update, reconnecting (and reporting offline) when
/// the socket drops. Runs for the app's lifetime.
pub fn spawn_status_emitter(app: AppHandle) {
    thread::spawn(move || loop {
        if let Err(err) = stream_status(&app) {
            let _ = app.emit(
                EVENT,
                AgentsEvent {
                    connected: false,
                    status: None,
                    error: Some(err.to_string()),
                },
            );
        }
        thread::sleep(RECONNECT_DELAY);
    });
}

/// Subscribes and emits each pushed update until the daemon closes the stream.
fn stream_status(app: &AppHandle) -> Result<()> {
    let (mut writer, mut reader) = connect(false)?;
    send(&mut writer, b"{\"type\":\"subscribe\"}\n")?;

    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line).map_err(socket("read"))? == 0 {
            return Ok(()); // the daemon closed the stream
        }
        let Ok(value) = serde_json::from_str::<Value>(line.trim()) else {
            continue; // skip a malformed frame, keep streaming
        };
        if let Ok(status) = status_from_value(&value) {
            let _ = app.emit(
                EVENT,
                AgentsEvent {
                    connected: true,
                    status: Some(status),
                    error: None,
                },
            );
        }
    }
}

/// Sends one request and requires an `ok` response.
fn request_ok(payload: &Value) -> Result<()> {
    let response = request(payload)?;
    match response.get("type").and_then(Value::as_str) {
        Some("ok") => Ok(()),
        Some("error") => Err(Error::Daemon(message_of(&response))),
        _ => Err(Error::UnexpectedResponse),
    }
}

/// Sends one request and returns the parsed JSON response.
fn request(payload: &Value) -> Result<Value> {
    let (mut writer, mut reader) = connect(true)?;
    let mut request = payload.to_string();
    request.push('\n');
    send(&mut writer, request.as_bytes())?;
    let mut line = String::new();
    reader.read_line(&mut line).map_err(socket("read"))?;
    serde_json::from_str(line.trim()).map_err(Error::BadResponse)
}

/// Opens the control socket as a (writer, line reader) pair. `hint` adds the
/// "try `beagle agent start`" suffix to the unreachable message.
fn connect(hint: bool) -> Result<(UnixStream, BufReader<UnixStream>)> {
    let path = beagle::agentd::socket_path();
    let stream = match UnixStream::connect(&path) {
        Ok(stream) => stream,
        Err(source) => return Err(Error::DaemonUnreachable { path, source, hint }),
    };
    let writer = stream.try_clone().map_err(socket("error"))?;
    Ok((writer, BufReader::new(stream)))
}

/// Writes and flushes one frame.
fn send(writer: &mut UnixStream, frame: &[u8]) -> Result<()> {
    writer.write_all(frame).map_err(socket("write"))?;
    writer.flush().map_err(socket("flush"))
}

/// Maps an I/O error on step `op` of an open connection to [`Error::Socket`].
fn socket(op: &'static str) -> impl FnOnce(std::io::Error) -> Error {
    move |source| Error::Socket { op, source }
}

/// Extracts an [`AgentsStatus`] from a `status` or `update` response, turning an
/// `error` response into an `Err`.
fn status_from_value(value: &Value) -> Result<AgentsStatus> {
    match value.get("type").and_then(Value::as_str) {
        Some("status" | "update") => {
            let status = value.get("status").ok_or(Error::MissingStatus)?;
            serde_json::from_value(status.clone()).map_err(Error::BadStatus)
        }
        Some("error") => Err(Error::Daemon(message_of(value))),
        _ => Err(Error::UnexpectedResponse),
    }
}

/// The `message` field of an error response, or a default.
fn message_of(value: &Value) -> String {
    value
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("daemon error")
        .to_string()
}
