//! Rendering an attached session ([`crate::transcript`] entries) into the
//! view the TUI's Session tab shows. The pipeline ([`crate::transcript::etl`])
//! writes it to `<session>.log` as ANSI text; the TUI only reads it back
//! with [`from_ansi`].
//!
//! ```text
//! ━━ session 1163b504 · 15:02 → 16:40 UTC ━━
//! 24 prompts · 140 commands (3 failed) · 5 files changed
//!   ✚ aws-bootstrap/scripts/bootstrap.sh     +270          what it made, first
//!   ✎ aws-bootstrap/terraform/main.tf        +12 −3
//! ── timeline ──
//! › You · 15:04 UTC                                         cyan     your prompt
//!   fix the flaky uploader test
//! ∴ thought for 4.0s                                        magenta  thinking
//! ● The image built. Now checking inside it.                white    Claude's reply
//! ▸ Verify bootstrap contents and terraform validate        green    what a command is for…
//!   $ docker run --rm --platform linux/amd64 --entrypo…     dim      …and the command, one line
//!   │ files: 19 .tf, tfvars: 0                              gray     output, ≤ 6 lines
//!   ✖ exit 2                                                red      failures
//! ✎ edited aws-bootstrap/terraform/main.tf  +12 −3          yellow   file changes
//! ▸ looked at main.tf, variables.tf, outputs.tf             blue     reads, grouped
//! ▶ task 04-runner-image · PR: ship the runner image        orange   scout task changes
//! ✎ image built: 19 .tf, terraform validate OK              cyan     scout log narration
//! ```

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};

use crate::markdown;
use crate::model::TaskStatus;
use crate::store::Store;
use crate::transcript::Entry;
use crate::watch::{scout_call, ScoutCall};

/// Output lines shown per command: the first [`HEAD`] and last
/// `MAX_OUTPUT - HEAD` when longer.
const MAX_OUTPUT: usize = 6;
const HEAD: usize = 3;
/// Commands are shown on one line, cut at this many characters.
const COMMAND_WIDTH: usize = 110;
/// Files listed in the summary before "and N more".
const SUMMARY_FILES: usize = 40;

const ORANGE: Color = Color::Indexed(208);

fn fg(color: Color) -> Style {
    Style::default().fg(color)
}

fn dim() -> Style {
    Style::default().fg(Color::DarkGray)
}

/// Renders sessions (`(session id, entries)`, oldest first). `store` titles
/// scout task banners.
#[must_use]
pub fn render(sessions: &[(String, Vec<Entry>)], store: &Store) -> Text<'static> {
    let mut lines = Vec::new();
    for (id, entries) in sessions {
        if !lines.is_empty() {
            lines.push(Line::default());
        }
        let cwd = entries.iter().find_map(|e| match e {
            Entry::Prompt { cwd: Some(cwd), .. } => Some(cwd.clone()),
            _ => None,
        });
        let mut r = Renderer {
            lines: Vec::new(),
            store,
            cwd,
            home: beagle_utils::paths::home_dir()
                .map(|h| h.to_string_lossy().into_owned())
                .unwrap_or_default(),
            tools: HashMap::new(),
            quiet: HashSet::new(),
            reads: Vec::new(),
        };
        r.summary(id, entries);
        for entry in entries {
            r.entry(entry);
        }
        r.flush_reads();
        lines.append(&mut r.lines);
    }
    Text::from(lines)
}

struct Renderer<'a> {
    lines: Vec<Line<'static>>,
    store: &'a Store,
    /// The session's working directory: paths are shown relative to it.
    cwd: Option<String>,
    home: String,
    /// tool-use id → tool name.
    tools: HashMap<String, String>,
    /// Tool uses shown as scout banners (their output is noise).
    quiet: HashSet<String>,
    /// Consecutive reads/searches, shown as one line when something else
    /// comes along.
    reads: Vec<String>,
}

/// One file's changes across the session.
struct FileChange {
    path: String,
    created: bool,
    added: usize,
    removed: usize,
}

/// `repo/inner/path` for a file inside a git repository (the nearest
/// ancestor holding `.git`), checked on disk.
fn repo_relative(path: &str) -> Option<String> {
    let path = std::path::Path::new(path);
    let root = path
        .ancestors()
        .skip(1)
        .find(|dir| dir.join(".git").exists())?;
    let name = root.file_name()?.to_string_lossy();
    let inner = path.strip_prefix(root).ok()?;
    Some(format!("{name}/{}", inner.display()))
}

fn is_lookup(name: &str) -> bool {
    matches!(name, "Read" | "Grep" | "Glob")
}

fn counts(added: usize, removed: usize) -> String {
    match (added, removed) {
        (a, 0) => format!("+{a}"),
        (0, r) => format!("−{r}"),
        (a, r) => format!("+{a} −{r}"),
    }
}

fn file_name(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_owned()
}

/// The first line of `text`, cut to `max` characters; marks what was cut.
fn one_line(text: &str, max: usize) -> String {
    let mut lines = text.lines();
    let first = lines.next().unwrap_or_default().trim_end();
    let more = lines.count();
    let mut out: String = first.chars().take(max).collect();
    if first.chars().count() > max {
        out.push('…');
    }
    if more > 0 {
        let _ = write!(out, "  (+{more} lines)");
    }
    out
}

/// `HH:MM UTC` from an RFC 3339 timestamp, or empty.
fn clock(ts: &str) -> String {
    ts.get(11..16)
        .filter(|hm| hm.as_bytes().get(2) == Some(&b':'))
        .map_or_else(String::new, |hm| format!("{hm} UTC"))
}

/// `MM-DD HH:MM UTC`, for spans across days.
fn day_clock(ts: &str) -> String {
    match ts.get(5..10) {
        Some(day) if day.as_bytes().get(2) == Some(&b'-') => format!("{day} {}", clock(ts)),
        _ => clock(ts),
    }
}

fn seconds(ms: u64) -> String {
    if ms < 1000 {
        format!("{ms}ms")
    } else {
        let tenths = ms / 100;
        format!("{}.{}s", tenths / 10, tenths % 10)
    }
}

mod ansi;
mod entries;
mod tools;

pub use ansi::{from_ansi, to_ansi};

#[cfg(test)]
mod tests;
