//! Recorded agent sessions: the terminal commands a Claude Code session ran,
//! captured by its hooks (`beagle scout hook`) and replayed by `beagle scout watch`.
//!
//! One JSONL file per session under `<root>/.sessions/`, one [`Event`] per
//! line, append-only. A command produces a `start` event (from the
//! `PreToolUse` hook) and then an `end` (`PostToolUse`) or a `fail`
//! (`PostToolUseFailure`), paired by the tool-use id.
//!
//! Appending to a file decouples the two sides: Claude never waits on a
//! watcher, and a watcher that starts late replays what it missed.

use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;

use crate::error::{Error, Result};

/// The directory under the store root holding session recordings. Hidden,
/// so the project listing never mistakes it for a project.
pub const SESSIONS_DIR: &str = ".sessions";

/// Output beyond this many bytes per stream is cut when recorded.
pub const MAX_OUTPUT_BYTES: usize = 64 * 1024;

/// Recordings untouched this long are deleted when a new session starts.
const KEEP_FOR: Duration = Duration::from_hours(336);

/// One recorded moment in a session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "kebab-case")]
pub enum Event {
    /// A command is about to run.
    Start {
        /// When the hook fired.
        #[serde(with = "time::serde::rfc3339")]
        ts: OffsetDateTime,
        /// The tool-use id pairing this start with its end.
        id: String,
        /// The command line, exactly as Claude wrote it.
        command: String,
        /// Claude's one-line description of the command.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        /// The session's working directory.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cwd: Option<String>,
        /// The subagent type, when a subagent ran it (e.g. `Explore`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        agent: Option<String>,
        /// Whether Claude ran it in the background.
        #[serde(default, skip_serializing_if = "is_false")]
        background: bool,
        /// The Herdr tab Claude runs in, if any.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tab: Option<String>,
        /// The Herdr pane Claude runs in, if any.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pane: Option<String>,
        /// Claude Code's transcript file, where the session's title lives.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        transcript: Option<String>,
    },
    /// A command finished and reported success.
    End {
        /// When the hook fired.
        #[serde(with = "time::serde::rfc3339")]
        ts: OffsetDateTime,
        /// The tool-use id of the matching start.
        id: String,
        /// Standard output (capped at [`MAX_OUTPUT_BYTES`]).
        #[serde(default)]
        stdout: String,
        /// Standard error (capped at [`MAX_OUTPUT_BYTES`]).
        #[serde(default)]
        stderr: String,
        /// Whether the command was interrupted.
        #[serde(default, skip_serializing_if = "is_false")]
        interrupted: bool,
        /// How long it ran.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        duration_ms: Option<u64>,
    },
    /// A command failed (non-zero exit, timeout, abort).
    Fail {
        /// When the hook fired.
        #[serde(with = "time::serde::rfc3339")]
        ts: OffsetDateTime,
        /// The tool-use id of the matching start.
        id: String,
        /// Claude Code's error text: `Exit code N` then the output.
        #[serde(default)]
        error: String,
        /// Whether it was an abort rather than a failure.
        #[serde(default, skip_serializing_if = "is_false")]
        interrupt: bool,
        /// How long it ran.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        duration_ms: Option<u64>,
    },
}

#[allow(clippy::trivially_copy_pass_by_ref)] // serde's skip_serializing_if passes a reference
fn is_false(b: &bool) -> bool {
    !*b
}

impl Event {
    /// The tool-use id the event belongs to.
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Self::Start { id, .. } | Self::End { id, .. } | Self::Fail { id, .. } => id,
        }
    }
}

/// Where Claude runs: the Herdr tab and pane from the hook's environment.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Location {
    /// `HERDR_TAB_ID`.
    pub tab: Option<String>,
    /// `HERDR_PANE_ID`.
    pub pane: Option<String>,
}

impl Location {
    /// Reads the Herdr ids from the environment.
    #[must_use]
    pub fn from_env() -> Self {
        let var = |name| std::env::var(name).ok().filter(|v| !v.is_empty());
        Self {
            tab: var("HERDR_TAB_ID"),
            pane: var("HERDR_PANE_ID"),
        }
    }
}

/// Turns one Claude Code hook payload into `(session id, event)`. Anything
/// that is not a Bash tool event, or lacks a usable session id, is `None`.
#[must_use]
pub fn event_from_hook(
    input: &Value,
    location: &Location,
    now: OffsetDateTime,
) -> Option<(String, Event)> {
    let text = |v: &Value, key: &str| v.get(key).and_then(Value::as_str).map(str::to_owned);
    let flag = |v: &Value, key: &str| v.get(key).and_then(Value::as_bool).unwrap_or(false);
    if input.get("tool_name").and_then(Value::as_str) != Some("Bash") {
        return None;
    }
    let session = text(input, "session_id").filter(|s| is_safe_id(s))?;
    let id = text(input, "tool_use_id").unwrap_or_default();
    let duration_ms = input.get("duration_ms").and_then(Value::as_u64);
    let event = match input.get("hook_event_name").and_then(Value::as_str)? {
        "PreToolUse" => {
            let tool_input = input.get("tool_input")?;
            Event::Start {
                ts: now,
                id,
                command: text(tool_input, "command")?,
                description: text(tool_input, "description").filter(|d| !d.is_empty()),
                cwd: text(input, "cwd"),
                agent: text(input, "agent_type"),
                background: flag(tool_input, "run_in_background"),
                tab: location.tab.clone(),
                pane: location.pane.clone(),
                transcript: text(input, "transcript_path"),
            }
        }
        "PostToolUse" => {
            let response = input.get("tool_response").unwrap_or(&Value::Null);
            Event::End {
                ts: now,
                id,
                stdout: cap(text(response, "stdout").unwrap_or_default()),
                stderr: cap(text(response, "stderr").unwrap_or_default()),
                interrupted: flag(response, "interrupted"),
                duration_ms,
            }
        }
        "PostToolUseFailure" => Event::Fail {
            ts: now,
            id,
            error: cap(text(input, "error").unwrap_or_default()),
            interrupt: flag(input, "is_interrupt"),
            duration_ms,
        },
        _ => return None,
    };
    Some((session, event))
}

/// Session ids become file names, so only `[A-Za-z0-9_-]` is accepted.
fn is_safe_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Cuts `text` to [`MAX_OUTPUT_BYTES`] on a character boundary, with a note.
fn cap(mut text: String) -> String {
    if text.len() <= MAX_OUTPUT_BYTES {
        return text;
    }
    let mut end = MAX_OUTPUT_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
    text.push_str("\n… [scout: output truncated]");
    text
}

/// The directory holding every session recording.
#[must_use]
pub fn dir(root: &Path) -> PathBuf {
    root.join(SESSIONS_DIR)
}

/// The recording file for `session`.
#[must_use]
pub fn file(root: &Path, session: &str) -> PathBuf {
    dir(root).join(format!("{session}.jsonl"))
}

/// Appends `event` to the session's recording in a single write, so
/// concurrent hooks never interleave inside a line. Starting a new
/// recording also prunes ones untouched for two weeks and, in a store
/// inside a git repo, makes sure its `.gitignore` keeps recordings out
/// (a store picked by `$SCOUT_ROOT` or config never ran `init`).
///
/// # Errors
/// [`Error::Io`] if the directory or file cannot be written.
pub fn append(root: &Path, session: &str, event: &Event) -> Result<()> {
    let dir = dir(root);
    fs::create_dir_all(&dir).map_err(Error::io(&dir))?;
    let path = file(root, session);
    if !path.exists() {
        prune(&dir);
        if crate::gitignore::in_git_repo(root) {
            crate::gitignore::ensure(root).map_err(Error::io(root.join(".gitignore")))?;
        }
    }
    let mut line = serde_json::to_string(event)?;
    line.push('\n');
    let mut out = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(Error::io(&path))?;
    out.write_all(line.as_bytes()).map_err(Error::io(&path))
}

fn prune(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let now = SystemTime::now();
    for path in entries.flatten().map(|e| e.path()) {
        let old = fs::metadata(&path)
            .and_then(|m| m.modified())
            .is_ok_and(|t| now.duration_since(t).unwrap_or_default() > KEEP_FOR);
        if old && path.extension().is_some_and(|e| e == "jsonl") {
            let _ = fs::remove_file(path); // best effort housekeeping
        }
    }
}

/// Parses one recorded line; `None` for blank or unreadable lines.
#[must_use]
pub fn parse_line(line: &str) -> Option<Event> {
    serde_json::from_str(line.trim()).ok()
}

mod listing;

pub use listing::{describe, list, pick, resolve, Description, SessionInfo};

#[cfg(test)]
mod tests;
