//! Key handling: map crossterm key events to dashboard actions.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// What a key press asks the app to do. Executed by `main`, which interprets
/// `Quit`/`Back` contextually (overlays close before the app exits).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// ctrl-c: exit unconditionally, whatever is on screen.
    ForceQuit,
    /// q: close the current overlay (help, log view), or exit from the
    /// dashboard.
    Quit,
    /// esc: close the current overlay; does nothing on the dashboard.
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
    Logs,
    /// y: copy — the whole pretty log in the log view, the run id on the
    /// dashboard.
    Copy,
    /// d: toggle the DAG page (projects → agents + run lineage).
    Dag,
    /// D: open the DAG page showing ONLY the selected run's chain.
    DagFocus,
    /// m: expand the selected run's detail into a floating pane.
    Detail,
    /// o: open the highlighted link (browser for a URL, editor for a file).
    Open,
    /// `/`: start typing a search term in the log view.
    Search,
    /// n / N: jump to the next / previous matching line.
    SearchNext,
    SearchPrev,
    /// e: edit the selected agent's manifest in $EDITOR (DAG page).
    Edit,
    /// f: cycle the RECENT state filter (all → succeeded → failed).
    Filter,
    Refresh,
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
        KeyCode::Char('y') => Action::Copy,
        KeyCode::Char('d') => Action::Dag,
        KeyCode::Char('D') => Action::DagFocus,
        KeyCode::Char('m') => Action::Detail,
        KeyCode::Char('o') => Action::Open,
        KeyCode::Char('e') => Action::Edit,
        KeyCode::Char('f') => Action::Filter,
        KeyCode::Char('/') => Action::Search,
        KeyCode::Char('n') => Action::SearchNext,
        KeyCode::Char('N') => Action::SearchPrev,
        KeyCode::Char('r') => Action::Refresh,
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
            Action::Refresh
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
