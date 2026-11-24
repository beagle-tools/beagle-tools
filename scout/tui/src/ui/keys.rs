//! Keypress → state transition. Bindings follow the standard beagle TUI
//! keymap (`docs/CODING_STANDARDS.md` "Terminal UIs"): `j`/`k` move, `enter`
//! opens, `b`/esc go back, `tab`/`1`–`5` switch tabs, `F` follows, `E`
//! edits, `r` reloads, `w` resizes the list, `?` helps, `q`/`Q`/ctrl-c
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
    ("q / Q / ctrl-c", "quit"),
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
            return if matches!(key.code, KeyCode::Char('q' | 'Q')) {
                Flow::Quit
            } else {
                Flow::Continue
            };
        }
        let half_page = i32::from(self.viewport_height / 2).max(1);
        match key.code {
            KeyCode::Char('q' | 'Q') => return Flow::Quit,
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
#[path = "keys_tests.rs"]
mod tests;
