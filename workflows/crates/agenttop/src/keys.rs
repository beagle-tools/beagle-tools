//! Key handling: map crossterm key events to actions, and the `?` sheet.
//!
//! The keys follow the standard keymap every beagle TUI shares
//! (`beagle_tui::keymap`, `docs/CODING_STANDARDS.md` "Terminal UIs"),
//! except `w`: agenttop has no side pane to resize, so `w` reads a step's
//! write-up. What each action does on each screen is `input.rs`.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// What a key press asks the app to do. `input::apply` interprets it for
/// the screen on top (overlays first).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// ctrl-c: exit unconditionally, whatever is on screen.
    ForceQuit,
    /// q / Q: quit, from any screen.
    Quit,
    /// esc / b: close the overlay on top, or go back a level.
    Back,
    Up,
    Down,
    /// g: the first row (the oldest line in a log).
    Top,
    /// G: the last row (the newest line in a log).
    Bottom,
    /// ctrl-d: half a screen down.
    PageDown,
    /// ctrl-u: half a screen up.
    PageUp,
    /// h / ←: pan the log view left (back toward the wrapped view).
    Left,
    /// →: pan the log view right (unwraps long lines). `l` doubles as this
    /// inside the log view.
    Right,
    /// enter: open — a job's runs, a step's log, a details link.
    Open,
    Kill,
    /// R: run the selected job, or resume the chain at the selected failed
    /// step — behind a confirmation pane (`confirm.rs`).
    Run,
    /// l: the selected step's log (pan right inside the log view).
    Logs,
    /// y: copy the selection — job name, run id, the whole log, a link.
    Copy,
    /// Y: copy the selected step's PR URL (or the details link).
    CopyUrl,
    /// i: copy the id — the job's name or the step's run id.
    CopyId,
    /// t (runs screen): attach to the selected running step's tmux session.
    Tmux,
    /// m: expand the selected run's detail into a floating pane.
    Detail,
    /// o: open in the browser — the step's draft PR, or a details link.
    OpenInBrowser,
    /// `w` — read the selected step's write-up, rendered.
    WriteUp,
    /// `/`: search the jobs list, or the log view.
    Search,
    /// n / N: jump to the next / previous matching line.
    SearchNext,
    SearchPrev,
    /// E: edit the selected agent's manifest in $EDITOR.
    Edit,
    /// r: re-read the store now (the screens also refresh every second).
    Reload,
    /// f (jobs, all-runs): only jobs or chains with something running.
    ShowRunning,
    /// a (jobs, all-runs): every job or chain again.
    ShowAll,
    /// H (jobs page): show / hide the projects `ui.yaml` hides.
    ToggleHidden,
    /// L (jobs page): every job's runs on one live page.
    AllRuns,
    /// W (runs screens): keep the cursor on the newest step as runs arrive.
    Follow,
    ToggleHelp,
    None,
}

/// The `?` sheet: `(keys, what they do)`, drawn by `chrome::draw_help`.
pub const HELP: &[(&str, &str)] = &[
    (
        "j/k ↑/↓",
        "move · g/G top/bottom · ctrl-d/ctrl-u half a page",
    ),
    ("enter", "open: a job's runs, a step's log, a details link"),
    (
        "esc / b",
        "back one level (on the jobs list: clear the search)",
    ),
    (
        "/",
        "search the jobs list or a log · n/N next/prev in a log",
    ),
    ("r", "reload now (the screens also refresh every second)"),
    ("f / a", "only running jobs (or chains) / all of them"),
    ("H", "show / hide the projects ui.yaml hides"),
    ("L", "every job's runs on one live page"),
    (
        "W",
        "watch: the cursor rides the newest step (moving stops it)",
    ),
    (
        "R",
        "run the job, or resume a failed step's chain (asks first)",
    ),
    ("x", "kill the selected step if it is running"),
    ("t", "attach to the step's tmux session (detach: ctrl-b d)"),
    ("w", "the step's write-up, rendered"),
    ("m", "the step's details and links"),
    (
        "o",
        "open the step's draft PR (or a details link) in the browser",
    ),
    ("E", "edit the selected job's manifest in $EDITOR"),
    (
        "y",
        "copy the selection: job name, run id, the whole log, a link",
    ),
    ("Y", "copy the step's PR URL"),
    ("i", "copy the id: the job's name or the step's run id"),
    ("l · h/←/→", "a step's log · pan a log left / right"),
    ("?", "this help (any key closes)"),
    ("q / Q", "quit (ctrl-c too)"),
];

pub fn map_key(key: KeyEvent) -> Action {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return match key.code {
            KeyCode::Char('c') => Action::ForceQuit,
            KeyCode::Char('d') => Action::PageDown,
            KeyCode::Char('u') => Action::PageUp,
            _ => Action::None,
        };
    }
    match key.code {
        KeyCode::Char('q' | 'Q') => Action::Quit,
        KeyCode::Esc | KeyCode::Char('b') => Action::Back,
        KeyCode::Char('j') | KeyCode::Down => Action::Down,
        KeyCode::Char('k') | KeyCode::Up => Action::Up,
        KeyCode::Char('g') => Action::Top,
        KeyCode::Char('G') => Action::Bottom,
        KeyCode::Char('h') | KeyCode::Left => Action::Left,
        KeyCode::Right => Action::Right,
        KeyCode::Enter => Action::Open,
        KeyCode::Char('x') => Action::Kill,
        KeyCode::Char('l') => Action::Logs,
        KeyCode::Char('w') => Action::WriteUp,
        KeyCode::Char('y') => Action::Copy,
        KeyCode::Char('Y') => Action::CopyUrl,
        KeyCode::Char('i') => Action::CopyId,
        KeyCode::Char('t') => Action::Tmux,
        KeyCode::Char('m') => Action::Detail,
        KeyCode::Char('o') => Action::OpenInBrowser,
        KeyCode::Char('E') => Action::Edit,
        KeyCode::Char('/') => Action::Search,
        KeyCode::Char('n') => Action::SearchNext,
        KeyCode::Char('N') => Action::SearchPrev,
        KeyCode::Char('r') => Action::Reload,
        KeyCode::Char('f') => Action::ShowRunning,
        KeyCode::Char('a') => Action::ShowAll,
        KeyCode::Char('H') => Action::ToggleHidden,
        KeyCode::Char('L') => Action::AllRuns,
        KeyCode::Char('W') => Action::Follow,
        KeyCode::Char('R') => Action::Run,
        KeyCode::Char('?') => Action::ToggleHelp,
        _ => Action::None,
    }
}
