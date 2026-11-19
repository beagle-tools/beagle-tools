//! The review TUI: the queue on the left (where each PR stands), the
//! selected PR on the right in three tabs — Summary, Diff, Comments.
//!
//! The UI thread owns one store connection and re-reads it every couple
//! of seconds, so a review the agentd job loads shows up without a key.
//! GitHub work (fetch, post, approve) runs on [`worker`] so the screen
//! never waits on the network.

mod diff;
mod keys;
mod markdown;
mod tabs;
mod term;
mod view;
mod worker;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};

use beagle_tui::{DefaultWidth, SideResize};
use tandem_cache::{Cache, ReviewStore, SummaryStore};
use tandem_core::github::PrDetail;
use tandem_core::review::{LocalComment, PrSummary};
use tandem_core::Result;

use crate::config::Config;
use crate::pr_ref::PrRef;
use crate::state::{self, Row};

pub use term::run;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Summary,
    Diff,
    Comments,
}

impl Tab {
    pub const ALL: [Self; 3] = [Self::Summary, Self::Diff, Self::Comments];

    pub fn title(self) -> &'static str {
        match self {
            Self::Summary => "Summary",
            Self::Diff => "Diff",
            Self::Comments => "Comments",
        }
    }

    fn step(self, forward: bool) -> Self {
        let i = Self::ALL.iter().position(|t| *t == self).unwrap_or(0);
        let n = Self::ALL.len();
        Self::ALL[if forward {
            (i + 1) % n
        } else {
            (i + n - 1) % n
        }]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    List,
    Content,
}

/// A GitHub write waiting for your yes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Post { comment_id: String, preview: String },
    Approve(PrRef),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Help,
    /// Typing a PR to queue.
    Input(String),
    Confirm(Action),
}

/// Everything shown for the selected PR.
#[derive(Debug, Clone)]
pub struct Loaded {
    pub pr: PrRef,
    pub detail: Option<PrDetail>,
    /// The commit the latest review read; comments anchor to it.
    pub head: Option<String>,
    pub raw_diff: Option<String>,
    /// Review comments, PR-level first, then by file and line.
    pub comments: Vec<LocalComment>,
    /// History replies by the id of the comment they sit under.
    pub replies: HashMap<String, Vec<LocalComment>>,
    pub summary: Option<PrSummary>,
}

pub struct App {
    pub cache: Cache,
    pub config: Config,
    pub rows: Vec<Row>,
    pub sel: usize,
    pub focus: Focus,
    pub tab: Tab,
    pub mode: Mode,
    pub loaded: Option<Loaded>,
    /// Scroll of the Summary and Diff tabs.
    pub scroll: usize,
    pub comment_sel: usize,
    pub detail_scroll: usize,
    /// Set by the last draw: how far the current tab can scroll.
    pub max_scroll: usize,
    pub page: usize,
    pub status: Option<(String, bool)>,
    pub tick: usize,
    /// The PR list hidden, so the diff gets the whole width.
    pub zoom: bool,
    /// The list's width and `w`'s resize mode (not saved across runs).
    pub pane: SideResize,
    pub diff: diff::DiffCache,
    worker: Option<Sender<worker::Cmd>>,
    done: Option<Receiver<worker::Done>>,
}

impl std::fmt::Debug for App {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("App")
            .field("sel", &self.sel)
            .finish_non_exhaustive()
    }
}

impl App {
    pub fn new(cache: Cache, config: Config) -> Self {
        Self {
            cache,
            config,
            rows: Vec::new(),
            sel: 0,
            focus: Focus::List,
            tab: Tab::Summary,
            mode: Mode::Normal,
            loaded: None,
            scroll: 0,
            comment_sel: 0,
            detail_scroll: 0,
            max_scroll: 0,
            page: 20,
            status: None,
            tick: 0,
            zoom: false,
            pane: list_pane(),
            diff: diff::DiffCache::default(),
            worker: None,
            done: None,
        }
    }

    /// Starts the GitHub worker on its own store connection.
    pub fn start_worker(&mut self, db: PathBuf) {
        let (tx, rx) = worker::spawn(db, self.config.clone());
        self.worker = Some(tx);
        self.done = Some(rx);
    }

    pub fn send(&mut self, cmd: worker::Cmd) {
        let sent = self.worker.as_ref().is_some_and(|w| w.send(cmd).is_ok());
        if !sent {
            self.error("the GitHub worker is not running");
        }
    }

    /// Takes finished GitHub work; true when anything finished.
    pub fn drain_worker(&mut self) -> bool {
        let Some(rx) = &self.done else {
            return false;
        };
        let finished: Vec<_> = rx.try_iter().collect();
        for done in &finished {
            match done {
                worker::Done::Ok(msg) => self.info(msg),
                worker::Done::Err(msg) => self.error(msg),
                worker::Done::Quiet => {}
            }
        }
        !finished.is_empty()
    }

    /// Asks the worker to re-check every PR GitHub could still change:
    /// anything not already merged or closed.
    pub fn check_states(&mut self) {
        let prs: Vec<PrRef> = self
            .rows
            .iter()
            .filter(|r| !r.stage.is_finished())
            .map(|r| r.pr.clone())
            .collect();
        if !prs.is_empty() {
            self.send(worker::Cmd::CheckStates(prs));
        }
    }

    pub fn info(&mut self, msg: impl Into<String>) {
        self.status = Some((msg.into(), false));
    }

    pub fn error(&mut self, msg: impl Into<String>) {
        self.status = Some((msg.into(), true));
    }

    pub fn selected(&self) -> Option<&Row> {
        self.rows.get(self.sel)
    }

    /// Re-reads the queue and the selected PR. Keeps the selection on
    /// the same PR when the list reorders.
    pub fn reload(&mut self) -> Result<()> {
        let keep = self.selected().map(|r| r.pr.clone());
        self.rows = state::rows(&self.cache)?;
        if let Some(pr) = keep {
            if let Some(i) = self.rows.iter().position(|r| r.pr == pr) {
                self.sel = i;
            }
        }
        self.sel = self.sel.min(self.rows.len().saturating_sub(1));
        self.load_selected()
    }

    fn load_selected(&mut self) -> Result<()> {
        let Some(pr) = self.selected().map(|r| r.pr.clone()) else {
            self.loaded = None;
            return Ok(());
        };
        if self.loaded.as_ref().is_some_and(|l| l.pr != pr) {
            self.scroll = 0;
            self.comment_sel = 0;
            self.detail_scroll = 0;
        }
        let detail = self.cache.get_pr_detail(&pr.repo, pr.number)?;
        let head = self
            .cache
            .list_agent_runs(&pr.repo, pr.number)?
            .into_iter()
            .filter(|r| !r.head_sha.is_empty())
            .max_by_key(|r| r.started_at)
            .map(|r| r.head_sha)
            .or_else(|| detail.as_ref().map(|d| d.pull_request.head_sha.clone()));
        let raw_diff = match &head {
            Some(h) => self.cache.get_raw_diff(&pr.repo, pr.number, h)?,
            None => None,
        };
        let all = self.cache.list_comments(&pr.repo, pr.number)?;
        let mut replies: HashMap<String, Vec<LocalComment>> = HashMap::new();
        let mut comments = Vec::new();
        for c in all {
            match &c.parent_id {
                Some(parent) => replies.entry(parent.clone()).or_default().push(c),
                None => comments.push(c),
            }
        }
        comments.sort_by(|a, b| {
            (!a.path.is_empty(), &a.path, a.line).cmp(&(!b.path.is_empty(), &b.path, b.line))
        });
        self.comment_sel = self.comment_sel.min(comments.len().saturating_sub(1));
        self.loaded = Some(Loaded {
            summary: self.cache.get_pr_summary(&pr.repo, pr.number)?,
            pr,
            detail,
            head,
            raw_diff,
            comments,
            replies,
        });
        Ok(())
    }

    pub fn comment(&self) -> Option<&LocalComment> {
        self.loaded.as_ref()?.comments.get(self.comment_sel)
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

/// The PR list's resize state: a third of the screen (30–56 columns) until
/// the user picks a width with `w`.
pub(crate) const fn list_pane() -> SideResize {
    SideResize::new(
        "list",
        DefaultWidth::Fraction {
            divisor: 3,
            min: 30,
            max: 56,
        },
    )
}
