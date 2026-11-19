//! Key handling: one state transition per key press.

use beagle_tui::pane_resize;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{App, Focus};
use crate::{clipboard, links};

/// The help sheet: `(keys, what they do)`.
pub(super) const HELP: &[(&str, &str)] = &[
    (
        "j / k  ↓ / ↑",
        "list: pick an item · write-up: move the line cursor",
    ),
    ("tab  enter", "switch focus: list ⇄ write-up"),
    ("esc", "end the selection, then back to the list"),
    ("J / K  space", "move through the write-up (from anywhere)"),
    ("ctrl-d / ctrl-u", "half page down / up"),
    ("g / G", "top / bottom"),
    ("v", "write-up: start / end a line selection"),
    (
        "y",
        "write-up: copy the line (or selection) · list: copy the ticket URL",
    ),
    ("Y", "copy the ticket's URL"),
    ("i", "copy the id (the date, or date + ticket)"),
    ("h / l  ← / →", "pan wide diagrams sideways"),
    ("d", "pick a report by day (j/k, enter)"),
    ("[ / ]", "older / newer report"),
    ("o", "open the ticket in the tracker"),
    ("p", "open the ticket's PRs"),
    ("r", "reload from disk"),
    pane_resize::HELP,
    ("?", "toggle this help"),
    ("q  ctrl-c", "quit"),
];

impl App {
    /// Applies one key press.
    pub fn handle_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl && key.code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }
        if self.side.is_active() {
            let total = self.side.last_total();
            self.status = self.side.handle_key(key, total).status;
            return;
        }
        if self.help {
            // Any key closes help; q still quits.
            self.help = false;
            if key.code == KeyCode::Char('q') {
                self.quit = true;
            }
            return;
        }
        if self.picker.is_some() {
            self.picker_key(key.code);
            return;
        }
        self.status = None;
        if self.view.nav_key(key.code, ctrl) {
            return;
        }
        match key.code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('?') => self.help = true,
            KeyCode::Char('w') => self.side.start(),
            KeyCode::Tab | KeyCode::Enter | KeyCode::BackTab => {
                self.focus = match self.focus {
                    Focus::List => Focus::Content,
                    Focus::Content => Focus::List,
                };
                // Entering the write-up puts the cursor on the first visible
                // line; leaving it ends any selection.
                if self.focus == Focus::Content {
                    self.view.enter();
                } else {
                    self.view.anchor = None;
                }
            }
            KeyCode::Esc if self.view.anchor.is_some() => self.view.anchor = None,
            KeyCode::Esc => self.focus = Focus::List,
            KeyCode::Char('j') | KeyCode::Down => self.down(1),
            KeyCode::Char('k') | KeyCode::Up => self.up(1),
            KeyCode::Char('v') if self.focus == Focus::Content => {
                self.status = self.view.toggle_visual().map(str::to_owned);
            }
            KeyCode::Char('y') if self.focus == Focus::Content => {
                self.status = Some(self.view.yank());
            }
            KeyCode::Char('d') => self.open_picker(),
            KeyCode::Char('[') => self.switch_report(1),
            KeyCode::Char(']') => self.switch_report(-1),
            KeyCode::Char('o') => self.open_ticket(),
            KeyCode::Char('i') => self.copy_id(),
            KeyCode::Char('p') => self.open_prs(),
            KeyCode::Char('y' | 'Y') => self.copy_url(),
            KeyCode::Char('r') => {
                self.reload();
                self.status = Some("reloaded".to_owned());
            }
            _ => {}
        }
    }

    fn down(&mut self, n: usize) {
        match self.focus {
            Focus::List => {
                self.select((self.selected + n).min(self.item_count().saturating_sub(1)));
            }
            Focus::Content => self.view.move_cursor(isize::try_from(n).unwrap_or(1)),
        }
    }

    fn up(&mut self, n: usize) {
        match self.focus {
            Focus::List => self.select(self.selected.saturating_sub(n)),
            Focus::Content => self.view.move_cursor(-isize::try_from(n).unwrap_or(1)),
        }
    }

    fn select(&mut self, index: usize) {
        if index != self.selected {
            self.selected = index;
            self.rebuild_doc();
        }
    }

    /// `delta` > 0 moves to an older report (dates are newest first).
    fn switch_report(&mut self, delta: isize) {
        let Some(next) = self
            .current
            .checked_add_signed(delta)
            .filter(|i| *i < self.dates.len())
        else {
            self.status = Some(
                if delta > 0 {
                    "no older report"
                } else {
                    "already on the newest report"
                }
                .to_owned(),
            );
            return;
        };
        self.go_to_report(next);
    }

    fn go_to_report(&mut self, index: usize) {
        if index != self.current {
            self.current = index;
            self.selected = 0;
            self.load_current();
        }
    }

    fn open_picker(&mut self) {
        match self.store.list() {
            Ok(rows) if rows.is_empty() => self.status = Some("no reports yet".to_owned()),
            Ok(rows) => {
                // Highlight the report on screen.
                let current = self.current_date().map(|d| d.to_string());
                let selected = rows
                    .iter()
                    .position(|r| Some(&r.date) == current.as_ref())
                    .unwrap_or(0);
                self.picker = Some(super::Picker { rows, selected });
            }
            Err(e) => self.status = Some(e.to_string()),
        }
    }

    fn picker_key(&mut self, code: KeyCode) {
        let Some(picker) = self.picker.as_mut() else {
            return;
        };
        let last = picker.rows.len().saturating_sub(1);
        match code {
            KeyCode::Char('j') | KeyCode::Down => picker.selected = (picker.selected + 1).min(last),
            KeyCode::Char('k') | KeyCode::Up => picker.selected = picker.selected.saturating_sub(1),
            KeyCode::Char('g') | KeyCode::Home => picker.selected = 0,
            KeyCode::Char('G') | KeyCode::End => picker.selected = last,
            KeyCode::Enter => {
                let date = picker.rows.get(picker.selected).map(|r| r.date.clone());
                self.picker = None;
                if let Some(i) =
                    date.and_then(|d| self.dates.iter().position(|x| x.to_string() == d))
                {
                    self.go_to_report(i);
                }
            }
            KeyCode::Char('q') => {
                self.picker = None;
                self.quit = true;
            }
            KeyCode::Esc | KeyCode::Char('d') => self.picker = None,
            _ => {}
        }
    }

    fn open_ticket(&mut self) {
        let Some(t) = self.selected_ticket() else {
            self.status = Some("select a ticket to open it in the tracker".to_owned());
            return;
        };
        let (id, url) = (t.id.to_string(), t.manifest.url.clone());
        self.status = Some(match url {
            Some(url) => match links::open_url(&url) {
                Ok(()) => format!("opened {id}"),
                Err(e) => e.to_string(),
            },
            None => format!("{id} has no url in ticket.toml"),
        });
    }

    fn open_prs(&mut self) {
        let Some(t) = self.selected_ticket() else {
            self.status = Some("select a ticket to open its PRs".to_owned());
            return;
        };
        let prs = t.manifest.prs.clone();
        if prs.is_empty() {
            self.status = Some(format!("{} has no PRs listed", t.id));
            return;
        }
        let failed = prs
            .iter()
            .filter_map(|pr| links::open_url(pr).err())
            .next_back();
        self.status = Some(match failed {
            Some(e) => e.to_string(),
            None => format!("opened {} PR(s)", prs.len()),
        });
    }

    /// `i`: copy the item's id — the report's date on the overview,
    /// `<date> <ticket>` on a ticket (a ticket can be in several reports;
    /// the date picks the write-up). Handy for pointing an agent at it.
    fn copy_id(&mut self) {
        let Some(date) = self.current_date() else {
            self.status = Some("no report on screen".to_owned());
            return;
        };
        let text = match self.selected_ticket() {
            Some(t) => format!("{date} {}", t.id),
            None => date.to_string(),
        };
        self.status = Some(match clipboard::copy(&text) {
            Ok(()) => format!("copied {text}"),
            Err(e) => e.to_string(),
        });
    }

    fn copy_url(&mut self) {
        let Some(t) = self.selected_ticket() else {
            self.status = Some("select a ticket to copy its url".to_owned());
            return;
        };
        let text = t.manifest.url.clone().unwrap_or_else(|| t.id.to_string());
        self.status = Some(match clipboard::copy(&text) {
            Ok(()) => format!("copied {text}"),
            Err(e) => e.to_string(),
        });
    }
}
