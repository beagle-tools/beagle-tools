//! Comment threads from GitHub, kept locally per recap, with what you have
//! read and what an agent has reviewed.
//!
//! Everything here is local and out of git (`recaps/*/comments/` is
//! ignored): it is re-fetchable from GitHub, or yours alone.
//!
//! ```text
//! recaps/<id>/comments/
//!   threads.json        every thread on the PR, as last fetched
//!   read.json           thread id → the last comment you have seen
//!   reviews/<thread>.md an agent's local review of the thread
//! recaps/.comments-error.txt   the last sync failure (gone after a success)
//! ```
//!
//! A thread is an inline review thread (with its replies), one conversation
//! comment, or a review's body. Comments from ignored authors (bots, CI,
//! integrations — see [`CommentsConfig`]) stay in a thread as context but
//! never count: a thread made only of them is dropped.

mod github;
mod local;
mod need;
mod sync;

use serde::{Deserialize, Serialize};

pub use github::{fetch, is_rate_limit, parse_threads, RATE_LIMIT_WAIT};
pub use local::{
    dir, error_path, is_read, is_reviewed, load, load_read, load_review, save_read, save_review,
    ReadMarks, Review, IN_SHORT,
};
pub use need::{marks, need, unix_of, Marks, Need, QUIET_SECS};
pub use sync::{pending, spawn_sync, spawn_sync_once, sync_all, Pending, SYNC_INTERVAL};

/// What to ignore, from `[comments]` in `~/.config/recap/config.toml`:
///
/// ```toml
/// [comments]
/// ignore_authors = ["dependabot[bot]", "github-actions[bot]", "some-ci-user"]
/// ignore_bots = true
/// # Text that marks a comment as an automated reply posted from your own
/// # account (e.g. a trailer your review agent adds). Hidden in the TUI.
/// automated_markers = ["Automated reply"]
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct CommentsConfig {
    /// Authors whose comments never count, by login.
    pub ignore_authors: Vec<String>,
    /// Also ignore every author GitHub marks as a bot.
    pub ignore_bots: bool,
    /// Substrings that mark a comment body as an automated reply (posted
    /// from a person's account by a tool). Empty by default.
    pub automated_markers: Vec<String>,
}

impl Default for CommentsConfig {
    fn default() -> Self {
        Self {
            ignore_authors: ["dependabot[bot]", "github-actions[bot]"]
                .map(str::to_owned)
                .to_vec(),
            ignore_bots: true,
            automated_markers: Vec::new(),
        }
    }
}

impl CommentsConfig {
    /// Whether a comment by `author` (a bot when `bot`) is ignored.
    #[must_use]
    pub fn ignores(&self, author: &str, bot: bool) -> bool {
        (self.ignore_bots && bot) || self.ignore_authors.iter().any(|a| a == author)
    }

    /// Whether `body` carries one of the [`Self::automated_markers`].
    #[must_use]
    pub fn is_automated(&self, body: &str) -> bool {
        self.automated_markers
            .iter()
            .any(|m| !m.is_empty() && body.contains(m.as_str()))
    }
}

/// One comment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Comment {
    /// GitHub's numeric id, as a string.
    pub id: String,
    /// Author login.
    pub author: String,
    /// GitHub marks the author a bot.
    pub bot: bool,
    /// An ignored author: shown as context, never counted.
    pub context: bool,
    /// RFC 3339.
    pub created: String,
    /// Markdown.
    pub body: String,
    /// Link to it on GitHub.
    pub url: String,
}

/// Where a thread lives on the PR.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ThreadKind {
    /// A review thread on a line of the diff.
    Inline {
        /// File path.
        path: String,
        /// Line, if GitHub still knows it.
        line: Option<u64>,
        /// Resolved on GitHub.
        resolved: bool,
        /// The code it was on has changed since.
        outdated: bool,
    },
    /// A comment in the conversation tab.
    Conversation,
    /// The body of a review (approve / request changes / comment).
    Review {
        /// `APPROVED`, `CHANGES_REQUESTED`, `COMMENTED`, …
        state: String,
    },
}

/// A thread: its first comment, then the replies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Thread {
    /// `t-<id>` (inline), `c-<id>` (conversation) or `r-<id>` (review).
    pub id: String,
    /// Where it lives.
    #[serde(flatten)]
    pub kind: ThreadKind,
    /// Oldest first.
    pub comments: Vec<Comment>,
}

impl Thread {
    /// The newest comment that counts (not context).
    #[must_use]
    pub fn last_counted(&self) -> Option<&Comment> {
        self.comments.iter().rev().find(|c| !c.context)
    }

    /// The newest comment by a person other than `pr_author` — what an
    /// agent review answers.
    #[must_use]
    pub fn last_from_others(&self, pr_author: &str) -> Option<&Comment> {
        self.comments
            .iter()
            .rev()
            .find(|c| !c.context && c.author != pr_author)
    }

    /// Where it is, for a heading: `path:line`, `conversation`, `review`.
    #[must_use]
    pub fn place(&self) -> String {
        match &self.kind {
            ThreadKind::Inline { path, line, .. } => match line {
                Some(l) => format!("{path}:{l}"),
                None => path.clone(),
            },
            ThreadKind::Conversation => "conversation".to_owned(),
            ThreadKind::Review { state } => format!("review · {}", state.to_lowercase()),
        }
    }
}

/// `threads.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Threads {
    /// When it was fetched (RFC 3339).
    pub fetched: String,
    /// Oldest first.
    pub threads: Vec<Thread>,
}

/// Keeps only threads where someone other than the PR's author spoke (not
/// counting ignored authors): a bot and the author's own (or automated)
/// reply to it is not a review for the author to read.
#[must_use]
pub fn from_others(threads: Vec<Thread>, pr_author: &str) -> Vec<Thread> {
    threads
        .into_iter()
        .filter(|t| t.last_from_others(pr_author).is_some())
        .collect()
}

/// Drops threads resolved on GitHub: settled, so no longer shown, read or
/// reviewed. Conversation comments and review bodies have no resolved
/// state and stay.
#[must_use]
pub fn open_only(threads: Vec<Thread>) -> Vec<Thread> {
    threads
        .into_iter()
        .filter(|t| !matches!(t.kind, ThreadKind::Inline { resolved: true, .. }))
        .collect()
}

#[cfg(test)]
#[path = "../tests/comments.rs"]
mod tests;
