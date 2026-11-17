//! Key handling: map crossterm key events to actions on the jobs screens.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// What a key press asks the app to do. Executed by `main`, which interprets
/// `Quit`/`Back` contextually (overlays close before the app exits).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// ctrl-c: exit unconditionally, whatever is on screen.
    ForceQuit,
    /// q: close the current overlay (help, log view), go back a level, or
    /// exit from the jobs list.
    Quit,
    /// esc: close the current overlay, or go back a level.
    Back,
    Up,
    Down,
    /// h / ←: pan the log view left (back toward the wrapped view).
    Left,
    /// →: pan the log view right (unwraps long lines). `l` doubles as this
    /// inside the log view.
    Right,
    Attach,
    Kill,
    /// R: run the selected job, or resume the chain at the selected failed
    /// step — behind a confirmation pane (`confirm.rs`).
    Run,
    Logs,
    /// y: copy — the whole pretty log in the log view.
    Copy,
    /// t (runs screen): attach to the selected running step's tmux session.
    Tmux,
    /// m: expand the selected run's detail into a floating pane.
    Detail,
    /// o: open the highlighted link (browser for a URL, editor for a file).
    Open,
    /// `w` — read the selected step's write-up, rendered.
    WriteUp,
    /// `/`: start typing a search term in the log view.
    Search,
    /// n / N: jump to the next / previous matching line.
    SearchNext,
    SearchPrev,
    /// e: edit the selected agent's manifest in $EDITOR (DAG page).
    Edit,
    /// r (jobs page): only jobs with something running.
    ShowRunning,
    /// a (jobs page): every job again.
    ShowAll,
    /// H (jobs page): show / hide the projects `ui.yaml` hides.
    ToggleHidden,
    /// A (jobs page): every job's runs on one live page.
    AllRuns,
    /// W (runs screens): keep the cursor on the newest step as runs arrive.
    Follow,
    ToggleHelp,
    None,
}

pub fn map_key(key: KeyEvent) -> Action {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return if key.code == KeyCode::Char('c') {
            Action::ForceQuit
        } else {
            Action::None
        };
    }
    match key.code {
        KeyCode::Char('q') => Action::Quit,
        KeyCode::Esc => Action::Back,
        KeyCode::Char('j') | KeyCode::Down => Action::Down,
        KeyCode::Char('k') | KeyCode::Up => Action::Up,
        KeyCode::Char('h') | KeyCode::Left => Action::Left,
        KeyCode::Right => Action::Right,
        KeyCode::Enter => Action::Attach,
        KeyCode::Char('x') => Action::Kill,
        KeyCode::Char('l') => Action::Logs,
        KeyCode::Char('w') => Action::WriteUp,
        KeyCode::Char('y') => Action::Copy,
        KeyCode::Char('t') => Action::Tmux,
        KeyCode::Char('m') => Action::Detail,
        KeyCode::Char('o') => Action::Open,
        KeyCode::Char('e') => Action::Edit,
        KeyCode::Char('/') => Action::Search,
        KeyCode::Char('n') => Action::SearchNext,
        KeyCode::Char('N') => Action::SearchPrev,
        KeyCode::Char('r') => Action::ShowRunning,
        KeyCode::Char('a') => Action::ShowAll,
        KeyCode::Char('H') => Action::ToggleHidden,
        KeyCode::Char('A') => Action::AllRuns,
        KeyCode::Char('W') => Action::Follow,
        KeyCode::Char('R') => Action::Run,
        KeyCode::Char('?') => Action::ToggleHelp,
        _ => Action::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    #[test]
    fn quit_keys() {
        assert_eq!(
            map_key(press(KeyCode::Char('q'), KeyModifiers::NONE)),
            Action::Quit
        );
        assert_eq!(
            map_key(press(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Action::ForceQuit
        );
        assert_eq!(
            map_key(press(KeyCode::Esc, KeyModifiers::NONE)),
            Action::Back
        );
    }

    #[test]
    fn movement_keys() {
        assert_eq!(
            map_key(press(KeyCode::Char('j'), KeyModifiers::NONE)),
            Action::Down
        );
        assert_eq!(
            map_key(press(KeyCode::Down, KeyModifiers::NONE)),
            Action::Down
        );
        assert_eq!(
            map_key(press(KeyCode::Char('k'), KeyModifiers::NONE)),
            Action::Up
        );
        assert_eq!(map_key(press(KeyCode::Up, KeyModifiers::NONE)), Action::Up);
    }

    #[test]
    fn action_keys() {
        assert_eq!(
            map_key(press(KeyCode::Enter, KeyModifiers::NONE)),
            Action::Attach
        );
        assert_eq!(
            map_key(press(KeyCode::Char('x'), KeyModifiers::NONE)),
            Action::Kill
        );
        assert_eq!(
            map_key(press(KeyCode::Char('l'), KeyModifiers::NONE)),
            Action::Logs
        );
        assert_eq!(
            map_key(press(KeyCode::Char('r'), KeyModifiers::NONE)),
            Action::ShowRunning
        );
        assert_eq!(
            map_key(press(KeyCode::Char('a'), KeyModifiers::NONE)),
            Action::ShowAll
        );
        assert_eq!(
            map_key(press(KeyCode::Char('H'), KeyModifiers::SHIFT)),
            Action::ToggleHidden
        );
        assert_eq!(
            map_key(press(KeyCode::Char('R'), KeyModifiers::SHIFT)),
            Action::Run
        );
        assert_eq!(
            map_key(press(KeyCode::Char('A'), KeyModifiers::SHIFT)),
            Action::AllRuns
        );
        assert_eq!(
            map_key(press(KeyCode::Char('W'), KeyModifiers::SHIFT)),
            Action::Follow
        );
        assert_eq!(
            map_key(press(KeyCode::Char('?'), KeyModifiers::SHIFT)),
            Action::ToggleHelp
        );
    }

    #[test]
    fn other_keys_do_nothing() {
        assert_eq!(
            map_key(press(KeyCode::Char('z'), KeyModifiers::NONE)),
            Action::None
        );
        assert_eq!(
            map_key(press(KeyCode::Char('x'), KeyModifiers::CONTROL)),
            Action::None
        );
    }
}
