//! The TUI: your PRs on the left, the selected PR on the right in three tabs
//! — **Summary** (the write-up), **Diff** (the change itself) and
//! **Comments** (people's threads, with agent reviews).
//!
//! All state lives in [`App`]; key handling ([`App::handle_key`]) is pure
//! state transition plus the occasional opener/clipboard call, so it is unit
//! tested without a terminal. Filesystem changes arrive from a `notify`
//! watcher over a channel and are coalesced into one reload per burst.

mod badges;
mod commands;
mod confirm;
mod doc;
mod event_loop;
mod jobs;
mod keys;
mod list;
mod status;
mod thread_state;
mod threads;
mod view;

use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};

use beagle_tui::DocView;

pub(crate) use jobs::{Confirm, Job};
pub(crate) use list::{list_entries, ListEntry};

use crate::comments::CommentsConfig;
use crate::error::Result;
use crate::model::RecapId;
use crate::poller::PrStatus;
use crate::store::{Recap, RecapSummary, Shelf, Store};

/// Which pane keys act on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    /// The PR list.
    List,
    /// The Summary / Diff pane.
    Content,
    /// Comments tab only: the agent review pane on the right.
    Review,
}

/// The content pane's tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    /// The write-up.
    Summary,
    /// The diff.
    Diff,
    /// The PR's comment threads from people, with agent reviews.
    Comments,
}

impl Tab {
    fn index(self) -> usize {
        match self {
            Self::Summary => 0,
            Self::Diff => 1,
            Self::Comments => 2,
        }
    }
}

/// The whole TUI state.
#[derive(Debug)]
// help / quit / use_delta / repoll / actions_enabled are independent flags,
// not the states of one machine.
#[allow(clippy::struct_excessive_bools)]
pub struct App {
    store: Store,
    /// Every recap, newest PR first.
    rows: Vec<RecapSummary>,
    /// Index into `rows`.
    selected: usize,
    recap: Option<Recap>,
    load_error: Option<String>,
    tab: Tab,
    focus: Focus,
    /// The content pane: the current tab's document and the position in it.
    view: DocView,
    /// Saved `(scroll, cursor)` per tab, so flipping tabs keeps your place.
    tab_pos: [(u16, usize); 3],
    /// Diff tab: `doc` index of each file header, for `n` / `N`.
    file_starts: Vec<usize>,
    /// Comments tab: the thread id of each sub-tab, in order.
    thread_ids: Vec<String>,
    /// The selected recap's read marks (`comments/read.json`).
    read_marks: crate::comments::ReadMarks,
    /// Unread / reviewed / pending thread counts per recap, for the list.
    comment_marks: HashMap<String, crate::comments::Marks>,
    /// The last comments sync failure, if it has not succeeded since.
    comments_error: Option<String>,
    /// `[comments]` from the config: what counts as a bot or automated reply.
    comments_config: CommentsConfig,
    /// The width the Comments tab was laid out for (it wraps its own text).
    comments_width: u16,
    /// Comments tab: which thread's sub-tab is shown.
    thread_index: usize,
    /// Comments tab: how far the review pane is scrolled.
    review_scroll: u16,
    status: Option<String>,
    /// Live PR status from the background poller, by PR URL.
    statuses: HashMap<String, PrStatus>,
    /// Render the Diff tab with the user's `delta` (side by side) when it's
    /// installed; else the built-in renderer.
    use_delta: bool,
    /// The width delta last rendered the Diff tab at (0 = not yet).
    delta_width: u16,
    /// Which recaps are listed: active, or the archive.
    shelf: Shelf,
    /// A GitHub action waiting for confirmation.
    confirm: Option<Confirm>,
    /// A background `gh` job in flight (its description), if any.
    busy: Option<String>,
    /// Background job results.
    jobs_tx: Sender<Job>,
    jobs_rx: Receiver<Job>,
    /// Ask the poller to re-check every PR now (after an action).
    repoll: bool,
    /// `false` in tests: actions are recorded in `requested` instead of
    /// running `gh`, so no test can ever touch GitHub.
    actions_enabled: bool,
    /// What would have run while actions are disabled.
    requested: Vec<String>,
    help: bool,
    quit: bool,
}

impl App {
    /// Opens the store on recap `id`, or the newest when `None`.
    ///
    /// # Errors
    /// [`crate::Error::Io`] if the store can't be listed.
    pub fn new(store: Store, id: Option<&RecapId>) -> Result<Self> {
        let (rows, warnings) = store.list()?;
        let (jobs_tx, jobs_rx) = mpsc::channel();
        let selected = id
            .and_then(|id| rows.iter().position(|r| r.id == id.as_str()))
            .unwrap_or(0);
        let mut app = Self {
            store,
            rows,
            selected,
            recap: None,
            load_error: None,
            tab: Tab::Summary,
            focus: Focus::List,
            view: DocView::new(),
            tab_pos: [(0, 0); 3],
            file_starts: Vec::new(),
            thread_ids: Vec::new(),
            read_marks: crate::comments::ReadMarks::new(),
            comment_marks: HashMap::new(),
            comments_error: None,
            comments_config: CommentsConfig::default(),
            comments_width: 80,
            thread_index: 0,
            review_scroll: 0,
            status: warnings.first().map(|w| format!("skipped a recap: {w}")),
            statuses: HashMap::new(),
            use_delta: crate::diff::delta_available(),
            delta_width: 0,
            shelf: Shelf::Active,
            confirm: None,
            busy: None,
            jobs_tx,
            jobs_rx,
            repoll: false,
            actions_enabled: true,
            requested: Vec::new(),
            help: false,
            quit: false,
        };
        if let Some(id) = id {
            if !app.rows.iter().any(|r| r.id == id.as_str()) {
                app.status = Some(format!("no recap {id}; showing the newest"));
            }
        }
        app.load_selected();
        app.refresh_marks();
        Ok(app)
    }

    /// Uses `config` to decide which comments are bots or automated
    /// replies in the Comments tab.
    pub fn set_comments_config(&mut self, config: CommentsConfig) {
        self.comments_config = config;
        self.rebuild_doc_keeping_view();
    }

    fn selected_row(&self) -> Option<&RecapSummary> {
        self.rows.get(self.selected)
    }

    fn load_selected(&mut self) {
        self.recap = None;
        self.load_error = None;
        let id = self.selected_row().map(|r| r.id.clone());
        if let Some(id) = id {
            match RecapId::new(&id).and_then(|id| self.store.load_from(self.shelf, &id)) {
                Ok(recap) => self.recap = Some(recap),
                Err(e) => self.load_error = Some(e.to_string()),
            }
        }
        self.tab_pos = [(0, 0); 3];
        self.thread_index = 0;
        self.review_scroll = 0;
        self.read_marks = self
            .selected_dir()
            .map(|d| crate::comments::load_read(&d))
            .unwrap_or_default();
        self.rebuild_doc();
    }

    /// Re-reads the store, keeping the same PR, tab and position.
    pub fn reload(&mut self) {
        let id = self.selected_row().map(|r| r.id.clone());
        match self.store.list_shelf(self.shelf) {
            Ok((rows, _)) => {
                if rows.len() > self.rows.len() {
                    if let Some(new) = rows
                        .iter()
                        .find(|r| !self.rows.iter().any(|o| o.id == r.id))
                    {
                        self.status = Some(format!("new recap: {}#{}", new.repo, new.number));
                    }
                }
                self.rows = rows;
            }
            Err(e) => self.status = Some(e.to_string()),
        }
        self.selected = id
            .as_ref()
            .and_then(|id| self.rows.iter().position(|r| &r.id == id))
            .unwrap_or(0)
            .min(self.rows.len().saturating_sub(1));
        let same = self.selected_row().map(|r| r.id.clone()) == id;
        // A live reload (an agent writing) must not throw the reader back to
        // the top: keep the tab, view and cursor where they were.
        let keep = (self.tab, self.view.position(), self.tab_pos);
        self.refresh_marks();
        self.load_selected();
        if same {
            let position;
            (self.tab, position, self.tab_pos) = keep;
            self.view.set_position(position);
            self.rebuild_doc_keeping_view();
        }
    }

    fn rebuild_doc(&mut self) {
        self.build_doc();
        self.view.reset_position();
    }

    fn rebuild_doc_keeping_view(&mut self) {
        let position = self.view.position();
        self.build_doc();
        self.view.set_position(position);
    }

    fn build_doc(&mut self) {
        let doc;
        (doc, self.file_starts) = match (&self.recap, self.tab) {
            (Some(r), Tab::Summary) => (doc::summary(r), Vec::new()),
            // delta needs the pane width: `layout` renders it on first draw.
            (Some(_), Tab::Diff) if self.use_delta => (Vec::new(), Vec::new()),
            (Some(r), Tab::Diff) => {
                let d = doc::diff(r);
                (d.lines, d.file_starts)
            }
            (Some(r), Tab::Comments) => match self.selected_dir() {
                Some(dir) => {
                    let t = threads::document(
                        r,
                        &dir,
                        &self.read_marks,
                        &self.comments_config,
                        self.comments_width,
                        self.thread_index,
                    );
                    self.thread_ids = t.ids;
                    (t.lines, Vec::new())
                }
                None => (Vec::new(), Vec::new()),
            },
            (None, _) => (Vec::new(), Vec::new()),
        };
        self.delta_width = 0;
        self.view.set_doc(doc);
    }

    fn set_tab(&mut self, tab: Tab) {
        if tab == self.tab {
            return;
        }
        self.tab_pos[self.tab.index()] = (self.view.scroll, self.view.cursor);
        self.tab = tab;
        // The review pane exists only on the Comments tab.
        if tab != Tab::Comments && self.focus == Focus::Review {
            self.focus = Focus::Content;
        }
        self.build_doc();
        (self.view.scroll, self.view.cursor) = self.tab_pos[tab.index()];
        self.view.pan = 0;
    }

    /// Renders the Diff tab with delta at `width`, if that's what's in use
    /// and it isn't already at this width. Falls back to the built-in
    /// renderer (for good) if delta fails.
    fn render_delta_at(&mut self, width: u16) {
        if self.tab != Tab::Diff || !self.use_delta || self.delta_width == width {
            return;
        }
        let Some(recap) = &self.recap else { return };
        let d = if let Some(d) = crate::diff::render_delta(&recap.diff, width) {
            d
        } else {
            self.use_delta = false;
            self.status = Some("delta failed; showing the built-in diff".to_owned());
            doc::diff(recap)
        };
        self.file_starts = d.file_starts;
        self.view.doc = d.lines;
        self.delta_width = width;
        self.view.laid_width = 0; // force the re-layout below
    }

    fn layout(&mut self, width: u16) {
        self.render_delta_at(width);
        // The Comments tab wraps its own text (so every row keeps its
        // gutter): rebuild it when the pane is a new width.
        if self.tab == Tab::Comments && self.comments_width != width {
            self.comments_width = width;
            self.rebuild_doc_keeping_view();
        }
        self.view.layout(width);
    }

    /// Whether the user asked to quit.
    #[must_use]
    pub fn should_quit(&self) -> bool {
        self.quit
    }
}

#[cfg(test)]
#[path = "../tests/ui.rs"]
mod tests;
#[cfg(test)]
#[path = "../tests/ui_actions.rs"]
mod tests_actions;
#[cfg(test)]
#[path = "../tests/ui_comments.rs"]
mod tests_comments;
#[cfg(test)]
#[path = "../tests/ui_status.rs"]
mod tests_status;
