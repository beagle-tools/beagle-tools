//! The event spool: every scout status event, one JSON line each, in
//! `$XDG_STATE_HOME/beagle/events/scout.jsonl`. `beagle serve` follows it
//! and pushes matching events to the phone (`docs/serve-push.md` at the
//! repo root is the contract).
//!
//! Written at the same moment as the macOS banner ([`crate::notify`]) and
//! with the same wording, but whatever `notify` / `[notify_events]` say:
//! those switch the Mac's banners, and the phone has its own settings.
//! `BEAGLE_EVENTS=0` turns the spool off. Like a banner, writing it never
//! fails the agent's command.

use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::config::{Config, NotifyEvents};
use crate::notify::{self, Event};

/// The record format's version (`v`).
pub const VERSION: u32 = 1;
/// The tool name in records and the spool's file name.
pub const TOOL: &str = "scout";
/// Past this size the spool is renamed to `<tool>.jsonl.1` before the next
/// append.
pub const MAX_SPOOL_BYTES: u64 = 1 << 20;
/// `body` is cut to this many characters, so a record stays one small write.
pub const MAX_BODY_CHARS: usize = 1000;

/// One event, as one line of the spool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    /// Format version ([`VERSION`]).
    pub v: u32,
    /// A random UUID (lowercase), unique per event.
    pub id: String,
    /// When it happened, RFC 3339 UTC.
    pub time: String,
    /// The tool (`scout`).
    pub tool: String,
    /// The event kind, one of [`CATALOG`]'s.
    pub kind: String,
    /// The project id.
    pub project: String,
    /// The project title.
    pub project_title: String,
    /// The task id (`NN-slug`); `None` for project events.
    pub task: Option<String>,
    /// The task title; `None` for project events.
    pub task_title: Option<String>,
    /// The banner's bold line.
    pub title: String,
    /// The banner's second line.
    pub subtitle: String,
    /// The banner's body.
    pub body: String,
    /// Where tapping it goes in the app: `beagle://scout/projects/<id>[/tasks/<task>]`.
    pub link: String,
}

/// One kind of event the phone can switch on or off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Kind {
    /// The name, as in `scout.toml`'s `[notify_events]`.
    pub kind: &'static str,
    /// What the app's switch says.
    pub label: &'static str,
    /// On for a newly registered phone (the same as scout's own default).
    pub default: bool,
    /// Plays a sound.
    pub sound: bool,
}

/// Scout's events, in the order the app lists them. The defaults equal
/// [`NotifyEvents::default`] (tested).
pub const CATALOG: [Kind; 6] = [
    Kind {
        kind: "blocked",
        label: "A task is blocked (needs you)",
        default: true,
        sound: true,
    },
    Kind {
        kind: "project_started",
        label: "A project starts",
        default: true,
        sound: false,
    },
    Kind {
        kind: "project_done",
        label: "A project is done",
        default: true,
        sound: false,
    },
    Kind {
        kind: "task_done",
        label: "Every finished task",
        default: false,
        sound: false,
    },
    Kind {
        kind: "task_started",
        label: "Every task an agent picks up (noisy)",
        default: false,
        sound: false,
    },
    Kind {
        kind: "task_paused",
        label: "A working task went quiet",
        default: true,
        sound: false,
    },
];

/// `event`'s kind name.
#[must_use]
pub fn kind(event: &Event) -> &'static str {
    match event {
        Event::ProjectStarted { .. } => "project_started",
        Event::ProjectDone { .. } => "project_done",
        Event::Blocked { .. } => "blocked",
        Event::TaskDone { .. } => "task_done",
        Event::TaskStarted { .. } => "task_started",
        Event::TaskPaused { .. } => "task_paused",
    }
}

/// The app link for a project, or one of its tasks.
#[must_use]
pub fn link(project: &str, task: Option<&str>) -> String {
    match task {
        Some(task) => format!("beagle://scout/projects/{project}/tasks/{task}"),
        None => format!("beagle://scout/projects/{project}"),
    }
}

/// The record for `event` in `project`; `task` is the task's `(id, title)`
/// for task events.
#[must_use]
pub fn record(
    project: (&str, &str),
    task: Option<(&str, &str)>,
    event: &Event,
    now: OffsetDateTime,
) -> Record {
    let (project_id, project_title) = project;
    let n = notify::render(&Config::default(), project_id, project_title, event);
    Record {
        v: VERSION,
        id: uuid::Uuid::new_v4().to_string(),
        time: now
            .replace_nanosecond(0)
            .ok()
            .and_then(|t| t.format(&Rfc3339).ok())
            .unwrap_or_default(),
        tool: TOOL.to_owned(),
        kind: kind(event).to_owned(),
        project: project_id.to_owned(),
        project_title: project_title.to_owned(),
        task: task.map(|(id, _)| id.to_owned()),
        task_title: task.map(|(_, title)| title.to_owned()),
        title: n.title,
        subtitle: n.subtitle,
        body: n.message.chars().take(MAX_BODY_CHARS).collect(),
        link: link(project_id, task.map(|(id, _)| id)),
    }
}

/// `$XDG_STATE_HOME/beagle/events` (or `~/.local/state/beagle/events`).
#[must_use]
pub fn dir() -> Option<PathBuf> {
    beagle_utils::paths::xdg_state_home().map(|d| d.join("beagle/events"))
}

/// The spool file for `tool` in `dir`.
#[must_use]
pub fn spool(dir: &Path, tool: &str) -> PathBuf {
    dir.join(format!("{tool}.jsonl"))
}

/// Appends `record` to its tool's spool in `dir`, rotating a full one first.
///
/// # Errors
/// The I/O error (callers outside tests ignore it, see [`emit`]).
pub fn append(dir: &Path, record: &Record) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let path = spool(dir, &record.tool);
    if std::fs::metadata(&path).is_ok_and(|m| m.len() >= MAX_SPOOL_BYTES) {
        std::fs::rename(&path, path.with_extension("jsonl.1"))?;
    }
    let mut line = serde_json::to_string(record).map_err(io::Error::other)?;
    line.push('\n');
    // One write of one line with O_APPEND, so agents writing at once never
    // interleave.
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    file.write_all(line.as_bytes())
}

/// Writes `record` to the spool, fire and forget: off with
/// `BEAGLE_EVENTS=0`, and every failure is silent.
pub fn emit(record: &Record) {
    if std::env::var("BEAGLE_EVENTS").is_ok_and(|v| v == "0") {
        return;
    }
    if let Some(dir) = dir() {
        let _ = append(&dir, record);
    }
}

/// Whether `kind` is on for a phone in scout's own defaults.
#[must_use]
pub fn default_for(events: &NotifyEvents, kind: &str) -> Option<bool> {
    Some(match kind {
        "blocked" => events.blocked,
        "project_started" => events.project_started,
        "project_done" => events.project_done,
        "task_done" => events.task_done,
        "task_started" => events.task_started,
        "task_paused" => events.task_paused,
        _ => return None,
    })
}

#[cfg(test)]
#[path = "tests/events.rs"]
mod tests;
