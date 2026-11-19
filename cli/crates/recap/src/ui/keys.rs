//! Key handling: one state transition per key press.

use beagle_tui::pane_resize;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{App, Focus, Tab};

/// The help sheet: `(keys, what they do)`.
pub(super) const HELP: &[(&str, &str)] = &[
    (
        "j / k  ↓ / ↑",
        "list: pick a PR · pane: move the line cursor",
    ),
    (
        "tab  enter",
        "switch focus: PR list → pane → (Comments) agent review",
    ),
    ("esc", "end the selection, then back to the list"),
    ("1 / 2 / 3", "Summary / Diff / Comments tab (and focus it)"),
    ("n / N", "next / previous file in the diff"),
    (
        "n / N  ] / [",
        "Comments: next / previous thread (sub-tabs) · J / K scroll its review",
    ),
    (
        "u",
        "Comments: mark the thread under the cursor unread (resting on one reads it)",
    ),
    ("J / K  space", "move through the pane (from anywhere)"),
    ("ctrl-d / ctrl-u", "half page down / up"),
    ("g / G", "top / bottom"),
    ("h / l  ← / →", "pan long lines sideways"),
    ("v", "pane: start / end a line selection"),
    (
        "y",
        "pane: copy the line (or selection) · list: copy the PR URL",
    ),
    ("Y", "copy the PR URL"),
    (
        "i",
        "copy the recap id (what every `beagle recap` command takes)",
    ),
    ("o", "open the PR on GitHub"),
    (
        "R",
        "draft → ready for review (on an open PR: back to draft)",
    ),
    (
        "M",
        "merge, if approved + all checks green (auto-merge while checks run)",
    ),
    ("X", "close the PR without merging, and archive its recap"),
    ("a", "archive this recap (local only; the PR is untouched)"),
    ("A", "show / hide the archive"),
    ("U", "in the archive: restore this recap"),
    ("t", "open the ticket in the tracker"),
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
        if self.confirm.is_some() {
            self.confirm_key(key.code);
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
        self.status = None;
        if self.focus == Focus::Review && self.review_key(key.code, ctrl) {
            return;
        }
        // Comments: J/K scroll the review pane beside the thread instead.
        let review_scroll =
            self.tab == Tab::Comments && matches!(key.code, KeyCode::Char('J' | 'K'));
        if !review_scroll && self.view.nav_key(key.code, ctrl) {
            return;
        }
        match key.code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('?') => self.help = true,
            KeyCode::Char('w') => self.side.start(),
            KeyCode::Tab | KeyCode::Enter | KeyCode::BackTab => self.toggle_focus(),
            KeyCode::Esc if self.view.anchor.is_some() => self.view.anchor = None,
            KeyCode::Esc => self.focus = Focus::List,
            // A tab key means "show me this": switch and focus the pane, so
            // j/k scroll it straight away.
            KeyCode::Char('1') => self.open_tab(Tab::Summary),
            KeyCode::Char('2') => self.open_tab(Tab::Diff),
            KeyCode::Char('3') => self.open_tab(Tab::Comments),
            KeyCode::Char('u') if !ctrl => self.mark_current_unread(),
            // Comments: n/N (or ]/[) switch thread sub-tabs; J/K scroll
            // the review pane beside it.
            KeyCode::Char('n' | ']') if self.tab == Tab::Comments => self.switch_thread(1),
            KeyCode::Char('N' | '[') if self.tab == Tab::Comments => self.switch_thread(-1),
            KeyCode::Char('J') if self.tab == Tab::Comments => self.scroll_review(3),
            KeyCode::Char('K') if self.tab == Tab::Comments => self.scroll_review(-3),
            KeyCode::Char('n') => self.jump_file(1),
            KeyCode::Char('N') => self.jump_file(-1),
            KeyCode::Char('j') | KeyCode::Down => self.down(),
            KeyCode::Char('k') | KeyCode::Up => self.up(),
            KeyCode::Char('v') if self.focus == Focus::Content => {
                self.status = self.view.toggle_visual().map(str::to_owned);
            }
            KeyCode::Char('y') if self.focus == Focus::Content => {
                self.status = Some(self.view.yank());
            }
            KeyCode::Char('y' | 'Y') => self.copy_pr_url(),
            KeyCode::Char('o') => self.open_pr(),
            KeyCode::Char('i') => self.copy_recap_id(),
            KeyCode::Char('R') => self.ask_ready(),
            KeyCode::Char('M') => self.ask_merge(),
            KeyCode::Char('X') => self.ask_close(),
            KeyCode::Char('a') => self.archive_selected(),
            KeyCode::Char('A') => self.toggle_shelf(),
            KeyCode::Char('U') => self.unarchive_selected(),
            KeyCode::Char('t') => self.open_ticket(),
            KeyCode::Char('r') => {
                self.reload();
                // Also fetch comments now, rather than at the next 5-minute pass.
                if self.actions_enabled {
                    crate::comments::spawn_sync_once(self.store.clone());
                }
                self.status = Some("reloaded · syncing comments in the background".to_owned());
            }
            _ => {}
        }
    }

    /// tab / enter: PR list → pane → (on Comments) the agent review → list.
    fn toggle_focus(&mut self) {
        match self.focus {
            Focus::List => self.focus_content(),
            Focus::Content if self.tab == Tab::Comments => {
                self.view.anchor = None;
                self.focus = Focus::Review;
                self.status = Some("agent review — j/k scroll · tab back to the list".to_owned());
            }
            Focus::Content | Focus::Review => {
                // Leaving the pane ends any selection.
                self.view.anchor = None;
                self.focus = Focus::List;
            }
        }
    }

    /// Keys while the agent review pane has focus: the movement keys scroll
    /// it. `false` for anything else, which then works as usual.
    fn review_key(&mut self, code: KeyCode, ctrl: bool) -> bool {
        let page = i16::try_from(self.view.viewport.saturating_sub(2))
            .unwrap_or(i16::MAX)
            .max(1);
        match code {
            KeyCode::Char('j') | KeyCode::Down => self.scroll_review(1),
            KeyCode::Char('k') | KeyCode::Up => self.scroll_review(-1),
            KeyCode::Char('d') if ctrl => self.scroll_review(page / 2),
            KeyCode::Char('u') if ctrl => self.scroll_review(-page / 2),
            KeyCode::Char(' ') | KeyCode::PageDown => self.scroll_review(page),
            KeyCode::PageUp => self.scroll_review(-page),
            KeyCode::Char('g') | KeyCode::Home => self.review_scroll = 0,
            KeyCode::Char('G') | KeyCode::End => self.review_scroll = u16::MAX,
            _ => return false,
        }
        true
    }

    fn open_tab(&mut self, tab: Tab) {
        self.set_tab(tab);
        self.focus_content();
    }

    /// Moves focus into the pane, putting the cursor on screen. A no-op when
    /// the pane already has it, so a live selection survives.
    fn focus_content(&mut self) {
        if self.focus == Focus::Content {
            return;
        }
        self.focus = Focus::Content;
        self.view.enter();
    }

    fn down(&mut self) {
        match self.focus {
            Focus::List => self.select((self.selected + 1).min(self.rows.len().saturating_sub(1))),
            Focus::Content => self.view.move_cursor(1),
            Focus::Review => self.scroll_review(1),
        }
    }

    fn up(&mut self) {
        match self.focus {
            Focus::List => self.select(self.selected.saturating_sub(1)),
            Focus::Content => self.view.move_cursor(-1),
            Focus::Review => self.scroll_review(-1),
        }
    }

    /// Picks another PR. It always opens on its Summary; the Diff is one key
    /// (`2`) away.
    fn select(&mut self, index: usize) {
        if index != self.selected {
            self.selected = index;
            self.tab = Tab::Summary;
            self.load_selected();
        }
    }

    /// Jumps to the next (`1`) or previous (`-1`) file in the diff, opening
    /// the Diff tab if needed. The file's header lands at the top of the view.
    fn jump_file(&mut self, delta: isize) {
        // In Comments, n/N walk the threads (their starts are in file_starts).
        let threads = self.tab == Tab::Comments;
        let unit = if threads { "thread" } else { "file" };
        if !threads && self.tab != Tab::Diff {
            self.set_tab(Tab::Diff);
        }
        if self.file_starts.is_empty() {
            self.status = Some(if threads {
                "no comment threads on this PR".to_owned()
            } else {
                "no files in this diff".to_owned()
            });
            return;
        }
        let current = self.view.owners.get(self.view.cursor).copied().unwrap_or(0);
        let target = if delta > 0 {
            self.file_starts.iter().copied().find(|&s| s > current)
        } else {
            self.file_starts
                .iter()
                .rev()
                .copied()
                .find(|&s| s < current)
        };
        let Some(doc_line) = target else {
            self.status = Some(format!(
                "{} {unit}",
                if delta > 0 { "last" } else { "first" }
            ));
            return;
        };
        if let Some(row) = self.view.owners.iter().position(|&o| o == doc_line) {
            self.view.cursor = row;
            self.view.scroll = u16::try_from(row)
                .unwrap_or(u16::MAX)
                .min(self.view.max_scroll());
            self.focus = Focus::Content;
            let n = self
                .file_starts
                .iter()
                .position(|&s| s == doc_line)
                .unwrap_or(0)
                + 1;
            self.status = Some(format!("{unit} {n}/{}", self.file_starts.len()));
        }
    }
}
