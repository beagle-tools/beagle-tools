//! Keypress → state transition. Bindings follow beagle's: `j`/`k` move,
//! `enter` opens, `b`/esc go back, `tab`/`1`–`3` switch tabs, `F` follows,
//! `E` edits, `r` reloads, `w` resizes the list, `?` helps, `Q`/ctrl-c
//! quit.

use std::path::PathBuf;

use beagle_tui::pane_resize;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{App, Focus, Screen, Tab};

/// What the event loop should do after a key.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Flow {
    /// Keep going.
    Continue,
    /// Exit the TUI.
    Quit,
    /// Suspend the TUI and open this file in the editor.
    Edit(PathBuf),
}

/// The help overlay's rows: `(keys, what they do)`.
pub(super) const HELP: &[(&str, &str)] = &[
    ("j / k  ↓ / ↑", "move the selection (or scroll the content)"),
    ("g / G", "first / last"),
    ("enter / l", "open the project, or read the task"),
    ("b / esc", "back: content → task list → projects"),
    ("tab / ] · shift-tab / [", "next / previous tab"),
    (
        "← / →",
        "previous / next tab (→ opens on the projects screen)",
    ),
    (
        "1 · 2 · 3 · 4 · 5",
        "Task · Outcome · Session · Project · Log tabs",
    ),
    ("space / pgdn · pgup", "page the content down / up"),
    ("ctrl-d / ctrl-u", "half-page the content, from anywhere"),
    (
        "F",
        "follow: keep the content pinned to its bottom (tail -f)",
    ),
    ("E", "open the shown file in $VISUAL / $EDITOR"),
    ("r", "reload from disk (it also reloads on its own)"),
    pane_resize::HELP,
    ("?", "this help"),
    ("Q / ctrl-c", "quit"),
];

impl App {
    /// Applies one key press.
    pub(super) fn handle_key(&mut self, key: KeyEvent) -> Flow {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl && key.code == KeyCode::Char('c') {
            return Flow::Quit;
        }
        self.status = None;
        if self.side.is_active() {
            let total = self.side.last_total();
            self.status = self.side.handle_key(key, total).status;
            return Flow::Continue;
        }
        if self.help {
            self.help = false;
            return if key.code == KeyCode::Char('Q') {
                Flow::Quit
            } else {
                Flow::Continue
            };
        }
        let half_page = i32::from(self.viewport_height / 2).max(1);
        match key.code {
            KeyCode::Char('Q') => return Flow::Quit,
            KeyCode::Char('?') => self.help = true,
            KeyCode::Char('w') => self.side.start(),
            KeyCode::Char('r') => {
                self.status = Some(self.reload().unwrap_or_else(|| "reloaded".to_owned()));
            }
            KeyCode::Char('E') => {
                if let Some(path) = self.current_file() {
                    return Flow::Edit(path);
                }
            }
            KeyCode::Char('F') => {
                self.follow = !self.follow;
                if self.follow {
                    self.scroll = usize::MAX;
                    self.status = Some("follow on: pinned to the bottom".to_owned());
                } else {
                    self.status = Some("follow off".to_owned());
                }
            }
            KeyCode::Char('d') if ctrl => self.scroll_by(half_page),
            KeyCode::Char('u') if ctrl => self.scroll_by(-half_page),
            _ => match self.screen {
                Screen::Projects => self.projects_key(key.code),
                Screen::Tasks => self.tasks_key(key.code),
            },
        }
        Flow::Continue
    }

    fn projects_key(&mut self, code: KeyCode) {
        let page = i32::from(self.viewport_height.saturating_sub(1)).max(1);
        match code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.select_project(self.project_sel.saturating_add(1));
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.select_project(self.project_sel.saturating_sub(1));
            }
            KeyCode::Char('g') | KeyCode::Home => self.select_project(0),
            KeyCode::Char('G') | KeyCode::End => self.select_project(usize::MAX),
            KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => self.open_project(),
            KeyCode::PageDown | KeyCode::Char(' ') => self.scroll_by(page),
            KeyCode::PageUp => self.scroll_by(-page),
            _ => {}
        }
    }

    fn tasks_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Tab | KeyCode::Char(']') | KeyCode::Right => self.tab = self.tab.step(true),
            KeyCode::BackTab | KeyCode::Char('[') | KeyCode::Left => {
                self.tab = self.tab.step(false);
            }
            KeyCode::Char(c @ '1'..='5') => {
                let index = usize::from(c as u8 - b'1');
                self.tab = Tab::ALL[index];
            }
            _ => match self.focus {
                Focus::List => self.task_list_key(code),
                Focus::Content => self.content_key(code),
            },
        }
    }

    fn task_list_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('j') | KeyCode::Down => self.select_task(self.task_sel.saturating_add(1)),
            KeyCode::Char('k') | KeyCode::Up => self.select_task(self.task_sel.saturating_sub(1)),
            KeyCode::Char('g') | KeyCode::Home => self.select_task(0),
            KeyCode::Char('G') | KeyCode::End => self.select_task(usize::MAX),
            KeyCode::Enter | KeyCode::Char('l') => self.focus = Focus::Content,
            KeyCode::Esc if self.follow => {
                self.follow = false;
                self.status = Some("follow off".to_owned());
            }
            KeyCode::Esc | KeyCode::Char('b' | 'h') => self.back_to_projects(),
            _ => {}
        }
    }

    fn content_key(&mut self, code: KeyCode) {
        let page = i32::from(self.viewport_height.saturating_sub(1)).max(1);
        match code {
            KeyCode::Char('j') | KeyCode::Down => self.scroll_by(1),
            KeyCode::Char('k') | KeyCode::Up => self.scroll_by(-1),
            KeyCode::Char(' ') | KeyCode::PageDown => self.scroll_by(page),
            KeyCode::PageUp => self.scroll_by(-page),
            KeyCode::Char('g') | KeyCode::Home => self.scroll = 0,
            KeyCode::Char('G') | KeyCode::End => self.scroll = self.max_scroll,
            KeyCode::Esc if self.follow => {
                self.follow = false;
                self.status = Some("follow off".to_owned());
            }
            KeyCode::Esc | KeyCode::Char('b' | 'h') => self.focus = Focus::List,
            _ => {}
        }
    }

    /// Scrolls the content by `delta` rendered lines. Scrolling up ends
    /// follow mode — the reader has left the bottom on purpose.
    fn scroll_by(&mut self, delta: i32) {
        let current = self.scroll.min(self.max_scroll);
        let step = usize::try_from(delta.unsigned_abs()).unwrap_or(usize::MAX);
        self.scroll = if delta < 0 {
            current.saturating_sub(step)
        } else {
            current.saturating_add(step).min(self.max_scroll)
        };
        if delta < 0 && self.follow {
            self.follow = false;
            self.status = Some("follow off".to_owned());
        }
    }
}

#[cfg(test)]
// Tests fail loudly on setup errors; unwrap/expect/panic is the clearest way.
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::model::{Slug, TaskStatus};
    use crate::store::Store;

    fn press(app: &mut App, code: KeyCode) -> Flow {
        app.handle_key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn app_with_project() -> (tempfile::TempDir, App) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        store
            .create_project(&Slug::parse("p").unwrap(), "P", vec![])
            .unwrap();
        for slug in ["a", "b", "c"] {
            store
                .add_task("p", &Slug::parse(slug).unwrap(), slug, vec![])
                .unwrap();
        }
        store.set_task_status("p", "a", TaskStatus::Done).unwrap();
        store
            .set_task_status("p", "b", TaskStatus::Working)
            .unwrap();
        let app = App::new(store).unwrap();
        (dir, app)
    }

    #[test]
    fn opening_a_project_lands_on_the_working_task() {
        let (_dir, mut app) = app_with_project();
        assert_eq!(press(&mut app, KeyCode::Enter), Flow::Continue);
        assert_eq!(app.screen, Screen::Tasks);
        assert_eq!(app.selected_task().unwrap().id, "02-b");
        assert_eq!(app.tab, Tab::Task);
    }

    #[test]
    fn back_peels_one_layer_at_a_time() {
        let (_dir, mut app) = app_with_project();
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.focus, Focus::Content);
        press(&mut app, KeyCode::Char('b'));
        assert_eq!((app.screen, app.focus), (Screen::Tasks, Focus::List));
        press(&mut app, KeyCode::Esc);
        assert_eq!(app.screen, Screen::Projects);
    }

    #[test]
    fn tabs_cycle_and_jump() {
        let (_dir, mut app) = app_with_project();
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Tab);
        assert_eq!(app.tab, Tab::Outcome);
        app.ensure_pane();
        assert!(
            app.current_file().unwrap().ends_with("02-b.md"),
            "the Outcome tab shows the task's own file"
        );
        press(&mut app, KeyCode::Tab);
        assert_eq!(app.tab, Tab::Session);
        press(&mut app, KeyCode::Tab);
        assert_eq!(app.tab, Tab::Project);
        press(&mut app, KeyCode::Char('5'));
        assert_eq!(app.tab, Tab::Log);
        press(&mut app, KeyCode::Tab);
        assert_eq!(app.tab, Tab::Task);
        press(&mut app, KeyCode::BackTab);
        assert_eq!(app.tab, Tab::Log);
        app.ensure_pane();
        assert!(app.current_file().unwrap().ends_with("log.md"));
    }

    #[test]
    fn selection_clamps() {
        let (_dir, mut app) = app_with_project();
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Char('G'));
        assert_eq!(app.selected_task().unwrap().id, "03-c");
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(app.selected_task().unwrap().id, "03-c");
        press(&mut app, KeyCode::Char('g'));
        press(&mut app, KeyCode::Char('k'));
        assert_eq!(app.selected_task().unwrap().id, "01-a");
    }

    #[test]
    fn follow_pins_and_scrolling_up_releases() {
        let (_dir, mut app) = app_with_project();
        app.max_scroll = 10;
        press(&mut app, KeyCode::Char('F'));
        assert!(app.follow);
        assert_eq!(app.scroll, usize::MAX);
        app.handle_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
        assert!(!app.follow);
        assert!(app.scroll < 10);
    }

    #[test]
    fn help_swallows_the_next_key() {
        let (_dir, mut app) = app_with_project();
        press(&mut app, KeyCode::Char('?'));
        assert!(app.help);
        press(&mut app, KeyCode::Enter);
        assert!(!app.help);
        assert_eq!(app.screen, Screen::Projects);
    }

    #[test]
    fn quit_and_edit() {
        let (_dir, mut app) = app_with_project();
        match press(&mut app, KeyCode::Char('E')) {
            Flow::Edit(path) => assert!(path.ends_with("project.md")),
            other => panic!("expected edit, got {other:?}"),
        }
        assert_eq!(press(&mut app, KeyCode::Char('Q')), Flow::Quit);
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(app.handle_key(ctrl_c), Flow::Quit);
    }

    #[test]
    fn reload_announces_status_changes_and_keeps_selection() {
        let (_dir, mut app) = app_with_project();
        press(&mut app, KeyCode::Enter);
        app.store
            .set_task_status("p", "c", TaskStatus::Working)
            .unwrap();
        let note = app.reload().unwrap();
        assert!(note.contains("03-c → working"), "{note}");
        assert_eq!(app.selected_task().unwrap().id, "02-b");
    }

    #[test]
    fn empty_store_is_harmless() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = App::new(Store::new(dir.path())).unwrap();
        for code in [
            KeyCode::Enter,
            KeyCode::Char('j'),
            KeyCode::Char('E'),
            KeyCode::Tab,
        ] {
            assert_eq!(press(&mut app, code), Flow::Continue);
        }
        assert_eq!(app.screen, Screen::Projects);
    }
}
