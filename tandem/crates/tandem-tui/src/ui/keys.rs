//! Keypress → state change. Bindings follow the standard beagle TUI keymap
//! (`docs/CODING_STANDARDS.md` "Terminal UIs"): j/k move, Enter/l opens,
//! Esc/h goes back, Tab and 1–3 switch tabs, r reloads, ? for help. Both
//! GitHub writes are uppercase (`P` post, `A` approve) and go through a y/n
//! confirmation first.

use beagle_tui::pane_resize;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tandem_cache::{ReviewStore, ReviewTriage};
use tandem_core::review::CommentStatus;

use super::worker::Cmd;
use super::{Action, App, Focus, Mode, Tab};
use crate::gh;
use crate::jobs;
use crate::pr_ref::PrRef;
use crate::state::Stage;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    Continue,
    Quit,
}

/// The help overlay, `(keys, what)`.
pub const HELP: &[(&str, &str)] = &[
    ("j/k  ↓/↑", "move (PRs, comments) or scroll"),
    ("Enter l / Esc h", "open the PR / back to the list"),
    ("Tab [ ]  1 2 3", "Summary · Diff · Comments"),
    ("Space PgDn  Ctrl-d/u", "page / half page"),
    ("g / G", "top / bottom"),
    ("n / N", "Diff: next / previous file"),
    ("Enter (Comments)", "show the comment's file in the Diff"),
    ("J / K", "Comments: scroll the selected comment"),
    ("P", "post the selected comment to GitHub (asks first)"),
    ("x", "dismiss / restore the selected comment"),
    ("A", "approve the PR on GitHub (asks first)"),
    ("m", "mark the PR gone-through / not"),
    ("a", "queue a PR (URL or owner/repo#n)"),
    ("e", "review the PR again"),
    ("z", "zoom: hide the list (full-width diff)"),
    pane_resize::HELP,
    ("o", "open the PR in the browser"),
    ("r", "refresh from GitHub"),
    ("?", "this help (any key closes it)"),
    ("q Q  Ctrl-c", "quit"),
];

impl App {
    pub fn handle_key(&mut self, key: KeyEvent) -> Flow {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl && key.code == KeyCode::Char('c') {
            return Flow::Quit;
        }
        if self.pane.is_active() {
            self.resize_key(key);
            return Flow::Continue;
        }
        match std::mem::replace(&mut self.mode, Mode::Normal) {
            // Any key closes help; q and Q still quit.
            Mode::Help if matches!(key.code, KeyCode::Char('q' | 'Q')) => return Flow::Quit,
            Mode::Help => {}
            Mode::Input(text) => self.input_key(key, text),
            Mode::Confirm(action) => self.confirm_key(key, action),
            Mode::Normal => return self.normal_key(key, ctrl),
        }
        Flow::Continue
    }

    fn normal_key(&mut self, key: KeyEvent, ctrl: bool) -> Flow {
        let half = (self.page / 2).max(1);
        match key.code {
            KeyCode::Char('q' | 'Q') => return Flow::Quit,
            KeyCode::Char('?') => self.mode = Mode::Help,
            KeyCode::Char('a') => self.mode = Mode::Input(String::new()),
            KeyCode::Char('A') => self.ask_approve(),
            KeyCode::Char('m') => self.toggle_done(),
            KeyCode::Char('e') => self.re_review(),
            KeyCode::Char('o') => self.open_in_browser(),
            KeyCode::Char('z') => self.zoom = !self.zoom,
            KeyCode::Char('w') => self.start_resize(),
            KeyCode::Char('r') => {
                if let Some(pr) = self.selected().map(|r| r.pr.clone()) {
                    self.send(Cmd::Fetch(pr));
                }
            }
            KeyCode::Tab | KeyCode::Char(']') => self.set_tab(self.tab.step(true)),
            KeyCode::BackTab | KeyCode::Char('[') => self.set_tab(self.tab.step(false)),
            KeyCode::Char(c @ '1'..='3') => {
                let i = usize::from(c as u8 - b'1');
                if let Some(tab) = Tab::ALL.get(i) {
                    self.set_tab(*tab);
                }
            }
            KeyCode::Char('d') if ctrl => self.scroll_by(half, true),
            KeyCode::Char('u') if ctrl => self.scroll_by(half, false),
            _ if self.focus == Focus::List => self.list_key(key),
            _ => self.content_key(key),
        }
        Flow::Continue
    }

    fn list_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => self.select(self.sel.saturating_add(1)),
            KeyCode::Char('k') | KeyCode::Up => self.select(self.sel.saturating_sub(1)),
            KeyCode::Char('g') | KeyCode::Home => self.select(0),
            KeyCode::Char('G') | KeyCode::End => self.select(usize::MAX),
            KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => {
                if self.selected().is_some() {
                    self.focus = Focus::Content;
                }
            }
            KeyCode::Char(' ') | KeyCode::PageDown => self.scroll_by(self.page, true),
            KeyCode::PageUp => self.scroll_by(self.page, false),
            _ => {}
        }
    }

    fn content_key(&mut self, key: KeyEvent) {
        let comments = self.tab == Tab::Comments;
        match key.code {
            KeyCode::Esc | KeyCode::Char('h' | 'b') | KeyCode::Left => self.focus = Focus::List,
            KeyCode::Char('j') | KeyCode::Down if comments => self.select_comment(true),
            KeyCode::Char('k') | KeyCode::Up if comments => self.select_comment(false),
            KeyCode::Char('J') if comments => self.scroll_by(1, true),
            KeyCode::Char('K') if comments => self.scroll_by(1, false),
            KeyCode::Char('P') if comments => self.ask_post(),
            KeyCode::Char('x') if comments => self.toggle_dismissed(),
            KeyCode::Enter if comments => self.show_in_diff(),
            KeyCode::Char('j') | KeyCode::Down => self.scroll_by(1, true),
            KeyCode::Char('k') | KeyCode::Up => self.scroll_by(1, false),
            KeyCode::Char(' ') | KeyCode::PageDown => self.scroll_by(self.page, true),
            KeyCode::PageUp => self.scroll_by(self.page, false),
            KeyCode::Char('g') | KeyCode::Home => self.set_scroll(0),
            KeyCode::Char('G') | KeyCode::End => self.set_scroll(usize::MAX),
            KeyCode::Char('n') if self.tab == Tab::Diff => self.jump_file(true),
            KeyCode::Char('N') if self.tab == Tab::Diff => self.jump_file(false),
            _ => {}
        }
    }

    fn input_key(&mut self, key: KeyEvent, mut text: String) {
        match key.code {
            KeyCode::Esc => {}
            KeyCode::Enter => self.queue(&text),
            KeyCode::Backspace => {
                text.pop();
                self.mode = Mode::Input(text);
            }
            KeyCode::Char(c) => {
                text.push(c);
                self.mode = Mode::Input(text);
            }
            _ => self.mode = Mode::Input(text),
        }
    }

    fn confirm_key(&mut self, key: KeyEvent, action: Action) {
        match key.code {
            KeyCode::Char('y' | 'Y') => match action {
                Action::Post { comment_id, .. } => {
                    self.info("posting…");
                    self.send(Cmd::Post(comment_id));
                }
                Action::Approve(pr) => {
                    self.info(format!("approving {}…", pr.label()));
                    self.send(Cmd::Approve(pr));
                }
            },
            KeyCode::Char('n' | 'N') | KeyCode::Esc => self.info("cancelled"),
            _ => self.mode = Mode::Confirm(action),
        }
    }

    fn select(&mut self, i: usize) {
        self.sel = i.min(self.rows.len().saturating_sub(1));
        if let Err(e) = self.reload() {
            self.error(e.to_string());
        }
    }

    fn set_tab(&mut self, tab: Tab) {
        self.tab = tab;
        self.scroll = 0;
        self.detail_scroll = 0;
    }

    fn scroll_by(&mut self, n: usize, down: bool) {
        let at = if self.tab == Tab::Comments {
            self.detail_scroll
        } else {
            self.scroll
        };
        let to = if down {
            at.saturating_add(n)
        } else {
            at.saturating_sub(n)
        };
        self.set_scroll(to);
    }

    fn set_scroll(&mut self, to: usize) {
        let to = to.min(self.max_scroll);
        if self.tab == Tab::Comments {
            self.detail_scroll = to;
        } else {
            self.scroll = to;
        }
    }

    fn select_comment(&mut self, down: bool) {
        let n = self.loaded.as_ref().map_or(0, |l| l.comments.len());
        self.comment_sel = if down {
            (self.comment_sel + 1).min(n.saturating_sub(1))
        } else {
            self.comment_sel.saturating_sub(1)
        };
        self.detail_scroll = 0;
    }

    fn jump_file(&mut self, forward: bool) {
        let starts: Vec<usize> = self.diff.last().files.iter().map(|(_, at)| *at).collect();
        let next = if forward {
            starts.iter().find(|s| **s > self.scroll + 2).copied()
        } else {
            starts.iter().rev().find(|s| **s + 2 < self.scroll).copied()
        };
        if let Some(at) = next {
            self.scroll = at;
        }
    }

    fn show_in_diff(&mut self) {
        let Some(path) = self.comment().map(|c| c.path.clone()) else {
            return;
        };
        self.tab = Tab::Diff;
        self.scroll = self.diff.last().file_start(&path).unwrap_or(0);
    }

    fn ask_post(&mut self) {
        let Some(c) = self.comment() else { return };
        if c.posted_github_id.is_some() {
            self.info("already posted");
            return;
        }
        let preview = gh::github_body(c);
        self.mode = Mode::Confirm(Action::Post {
            comment_id: c.id.clone(),
            preview,
        });
    }

    fn ask_approve(&mut self) {
        let Some(row) = self.selected() else { return };
        if matches!(row.stage, Stage::Queued | Stage::Reviewing) {
            self.error("the agent hasn't finished reviewing this PR yet");
            return;
        }
        self.mode = Mode::Confirm(Action::Approve(row.pr.clone()));
    }

    fn toggle_dismissed(&mut self) {
        let Some(c) = self.comment() else { return };
        let to = if c.status == CommentStatus::Rejected {
            CommentStatus::Open
        } else {
            CommentStatus::Rejected
        };
        let id = c.id.clone();
        let result = self
            .cache
            .set_comment_status(&id, to)
            .and_then(|()| self.reload());
        if let Err(e) = result {
            self.error(e.to_string());
        }
    }

    fn toggle_done(&mut self) {
        let Some(row) = self.selected() else { return };
        let (slug, n, done) = (row.pr.repo.slug(), row.pr.number, row.stage == Stage::Done);
        if !done && row.stage != Stage::Ready {
            self.error("only a reviewed PR can be marked gone-through");
            return;
        }
        let result = self
            .cache
            .set_reviewed(&slug, n, !done)
            .and_then(|()| self.reload());
        match result {
            Ok(()) if done => self.info("back to ready"),
            Ok(()) => self.info("marked gone-through"),
            Err(e) => self.error(e.to_string()),
        }
    }

    fn re_review(&mut self) {
        if let Some(pr) = self.selected().map(|r| r.pr.clone()) {
            self.queue(&pr.label());
        }
    }

    /// Queues a PR from the input line (or `e`), fetches it, and kicks
    /// the reviewer.
    fn queue(&mut self, text: &str) {
        let Some(pr) = PrRef::parse(text) else {
            self.error(format!("not a PR: {text:?} (URL or owner/repo#n)"));
            return;
        };
        if let Err(e) = jobs::add(&self.cache, &pr, "tui") {
            self.error(e.to_string());
            return;
        }
        self.send(Cmd::Fetch(pr.clone()));
        match crate::kick(&self.config) {
            Some(problem) => self.error(problem),
            None => self.info(format!("queued {}", pr.label())),
        }
        if let Err(e) = self.reload() {
            self.error(e.to_string());
        }
    }

    fn open_in_browser(&mut self) {
        let Some(url) = self.selected().map(|r| r.pr.url()) else {
            return;
        };
        // `open` on macOS, `xdg-open` elsewhere; detached but reaped.
        if let Err(e) = beagle_tui::links::open_url(&url) {
            self.error(format!("open {url}: {e}"));
        }
    }

    /// `w`: start resizing the list, unless it is zoomed away.
    fn start_resize(&mut self) {
        if self.zoom {
            self.info("unzoom (z) to resize the list");
        } else {
            self.pane.start();
            self.status = None;
        }
    }

    /// A key while resizing (see [`beagle_tui::pane_resize`]).
    fn resize_key(&mut self, key: KeyEvent) {
        let total = self.pane.last_total();
        match self.pane.handle_key(key, total).status {
            Some(msg) => self.info(msg),
            None => self.status = None,
        }
    }
}
