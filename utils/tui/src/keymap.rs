//! The standard keymap every beagle TUI follows, as data
//! (`docs/CODING_STANDARDS.md` "Terminal UIs").
//!
//! Three copies are kept in lock-step by tests, so none can drift:
//! - [`STANDARD`] here, which every TUI's keymap test walks with an
//!   exhaustive `match` on [`Job`] (a new job fails to compile until every
//!   TUI handles it or says why it does not apply);
//! - `standards/tui-standards.json`, checked against [`STANDARD`] by this
//!   module's tests;
//! - the table in `docs/CODING_STANDARDS.md`, checked against the JSON by
//!   `scripts/check-standards.mjs` in the always-run standards job.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// One row of the standard keymap table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Job {
    /// Move the selection or cursor.
    Move,
    /// Open the selected thing; go back out.
    OpenBack,
    /// Cycle tabs.
    Tabs,
    /// Show help; quit.
    HelpQuit,
    /// Reload from disk (or refresh from the source).
    Reload,
    /// Search the content.
    Search,
    /// Open the selected thing in the browser.
    OpenInBrowser,
    /// Open the shown file in the editor.
    Edit,
    /// Copy a line or selection, a URL, an id.
    Copy,
    /// Resize the side pane (`beagle_tui::pane_resize`).
    Resize,
}

/// A row: the job, the keys every TUI that has the job must honour (in the
/// order the tests press them), and the table's "Keys" cell.
#[derive(Debug, Clone, Copy)]
pub struct Binding {
    /// Which job.
    pub job: Job,
    /// The table's "Job" cell.
    pub name: &'static str,
    /// The keys a TUI's test presses for this job (see [`key`]).
    pub keys: &'static [&'static str],
    /// The table's "Keys" cell (Markdown).
    pub doc: &'static str,
}

/// The standard keymap, in table order.
pub const STANDARD: &[Binding] = &[
    Binding {
        job: Job::Move,
        name: "Move",
        keys: &["j", "k", "g", "G", "ctrl-d", "ctrl-u"],
        doc: "`j`/`k` or `↑`/`↓`, `g`/`G` top/bottom, `ctrl-d`/`ctrl-u` page",
    },
    Binding {
        job: Job::OpenBack,
        name: "Open / back",
        keys: &["Enter", "Esc"],
        doc: "`Enter`, `l` or `→` open; `Esc`, `b`, `h` or `←` back",
    },
    Binding {
        job: Job::Tabs,
        name: "Tabs",
        keys: &["Tab", "]", "["],
        doc: "`Tab`, `]` and `[` cycle tabs (and `1`–`9` jump)",
    },
    Binding {
        job: Job::HelpQuit,
        name: "Help / quit",
        keys: &["?", "q", "Q"],
        doc: "`?` help; `q` and `Q` quit; any key closes help",
    },
    Binding {
        job: Job::Reload,
        name: "Reload",
        keys: &["r"],
        doc: "`r`",
    },
    Binding {
        job: Job::Search,
        name: "Search",
        keys: &["/"],
        doc: "`/`",
    },
    Binding {
        job: Job::OpenInBrowser,
        name: "Open in browser",
        keys: &["o"],
        doc: "`o`",
    },
    Binding {
        job: Job::Edit,
        name: "Edit",
        keys: &["E"],
        doc: "`E`",
    },
    Binding {
        job: Job::Copy,
        name: "Copy",
        keys: &["y", "Y", "i"],
        doc: "`y` line or selection, `Y` URL, `i` id",
    },
    Binding {
        job: Job::Resize,
        name: "Resize side pane",
        keys: &["w"],
        doc: "`w` (above)",
    },
];

/// GitHub writes: `(key, action)`. Uppercase, the same letter in every
/// tool, always confirmed, never reused for a harmless action.
pub const GITHUB_WRITES: &[(&str, &str)] = &[
    ("A", "approve"),
    ("M", "merge"),
    ("P", "post/publish"),
    ("D", "ready↔draft"),
    ("X", "close"),
];

/// The row for `job`.
///
/// # Panics
/// Never: every [`Job`] has a row (a test checks it).
#[must_use]
#[allow(clippy::expect_used)] // covered by `every_job_has_a_row`
pub fn binding(job: Job) -> &'static Binding {
    STANDARD
        .iter()
        .find(|b| b.job == job)
        .expect("every job has a row")
}

/// The key events for `job`, in [`Binding::keys`] order.
#[must_use]
pub fn keys(job: Job) -> Vec<KeyEvent> {
    binding(job).keys.iter().map(|k| key(k)).collect()
}

/// A key spec from [`STANDARD`] as a key event: one character, a named key
/// (`Enter`, `Esc`, `Tab`, `BackTab`, `Up`, `Down`, `Left`, `Right`) or
/// `ctrl-<char>`. Anything else is a bug in the table and maps to `Null`.
#[must_use]
pub fn key(spec: &str) -> KeyEvent {
    if let Some(c) = spec.strip_prefix("ctrl-").and_then(single) {
        return KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL);
    }
    let code = match spec {
        "Enter" => KeyCode::Enter,
        "Esc" => KeyCode::Esc,
        "Tab" => KeyCode::Tab,
        "BackTab" => KeyCode::BackTab,
        "Up" => KeyCode::Up,
        "Down" => KeyCode::Down,
        "Left" => KeyCode::Left,
        "Right" => KeyCode::Right,
        other => single(other).map_or(KeyCode::Null, KeyCode::Char),
    };
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn single(s: &str) -> Option<char> {
    let mut chars = s.chars();
    let c = chars.next()?;
    chars.next().is_none().then_some(c)
}

/// The `standards/tui-standards.json` line for a row, exactly as written
/// there (one row per line).
#[must_use]
pub fn json_line(b: &Binding) -> String {
    let keys: Vec<String> = b.keys.iter().map(|k| format!("\"{k}\"")).collect();
    format!(
        "{{\"job\": \"{}\", \"keys\": [{}], \"doc\": \"{}\"}}",
        b.name,
        keys.join(", "),
        b.doc
    )
}

#[cfg(test)]
#[path = "tests/keymap.rs"]
mod tests;
