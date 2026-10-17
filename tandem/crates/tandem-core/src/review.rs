//! Local review state: comments written by you or an agent that live in
//! Tandem's cache and are NEVER posted to GitHub.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::github::RepoRef;

/// Who wrote a local comment — decides styling and whether agent-only
/// flows (re-review, replies) apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommentAuthorKind {
    /// Written by you in the UI.
    Human,
    /// Emitted by an agent run (`tandem_comment` blocks or MCP).
    Agent,
}

/// Triage state of a local comment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommentStatus {
    /// Freshly written, awaiting your triage.
    Open,
    /// You agreed with the comment.
    Accepted,
    /// You rejected the comment (kept for the record).
    Rejected,
    /// Addressed / no longer relevant.
    Resolved,
    /// Kept for the record but hidden from the active review.
    Archived,
}

/// How much a comment should block the PR, as judged by its author.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommentSeverity {
    /// Context or an FYI; nothing to change.
    Info,
    /// Optional improvement.
    Suggestion,
    /// A real problem that should be fixed.
    Issue,
    /// Must be fixed before merging.
    Blocker,
}

/// Which side of the diff the comment anchors to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffSide {
    /// Left-hand side: the base version (removed or context lines).
    Old,
    /// Right-hand side: the head version (added or context lines).
    New,
}

/// A local-only review comment, anchored to a line in a PR diff.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalComment {
    /// UUID string assigned by the cache layer.
    pub id: String,
    /// Repository of the PR the comment is on.
    pub repo: RepoRef,
    /// PR number within [`LocalComment::repo`].
    pub pr_number: u64,
    /// Head SHA the comment was written against (comments can go stale).
    pub head_sha: String,
    /// Repo-relative file path the comment is anchored to.
    pub path: String,
    /// Side of the diff [`LocalComment::line`] counts on.
    pub side: DiffSide,
    /// 1-based line number on [`LocalComment::side`]; start of the range for
    /// multi-line anchors.
    pub line: u64,
    /// Inclusive end of a multi-line anchor ("lines 2-5"); None for a
    /// single line.
    #[serde(default)]
    pub end_line: Option<u64>,
    /// Markdown body.
    pub body: String,
    /// Concrete replacement for lines `line..=end_line` (new side) — the
    /// agent's proposed fix, committable to the PR branch.
    #[serde(default)]
    pub suggestion: Option<String>,
    /// Human or agent — see [`LocalComment::author_name`] for which one.
    pub author_kind: CommentAuthorKind,
    /// Human username or agent name (e.g. "claude", "codex").
    pub author_name: String,
    /// Author-assigned severity.
    pub severity: CommentSeverity,
    /// Current triage state.
    pub status: CommentStatus,
    /// Agent run that produced this comment, if any.
    pub run_id: Option<String>,
    /// Commit this suggestion became, once you committed it. Proof it
    /// landed, and a link to what landed.
    #[serde(default)]
    pub committed_sha: Option<String>,
    /// How many times the body has been rewritten. The versions
    /// themselves live in `comment_revisions`; this is here so a list
    /// can show "edited" without a query per comment.
    #[serde(default)]
    pub edit_count: u32,
    /// What kind of remark this is — "coding-standards" when it cites
    /// the repo's own conventions, "correctness", "tests", and so on.
    /// Free-form on purpose: an agent knows its reasons better than a
    /// fixed enum would.
    #[serde(default)]
    pub tag: Option<String>,
    /// Root comment this replies to; None for top-level comments.
    #[serde(default)]
    pub parent_id: Option<String>,
    /// GitHub review-comment id this thread discusses, when the thread
    /// was started from a GitHub comment.
    #[serde(default)]
    pub github_comment_id: Option<u64>,
    /// Set once this comment's body has been posted to GitHub (the id
    /// GitHub assigned). Posting is always an explicit user action.
    #[serde(default)]
    pub posted_github_id: Option<u64>,
    /// When the comment was created.
    pub created_at: DateTime<Utc>,
    /// Last change to body or status.
    pub updated_at: DateTime<Utc>,
}

/// Fields callers provide when creating a comment; the cache layer fills
/// in id and timestamps.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewLocalComment {
    /// Repository of the PR.
    pub repo: RepoRef,
    /// PR number within [`NewLocalComment::repo`].
    pub pr_number: u64,
    /// Head SHA the comment is written against.
    pub head_sha: String,
    /// Repo-relative file path.
    pub path: String,
    /// Side of the diff the line numbers count on.
    pub side: DiffSide,
    /// 1-based anchor line (range start).
    pub line: u64,
    /// Inclusive range end; `None` for a single line.
    #[serde(default)]
    pub end_line: Option<u64>,
    /// Markdown body.
    pub body: String,
    /// Proposed replacement text for the anchored lines (new side).
    #[serde(default)]
    pub suggestion: Option<String>,
    /// Human or agent.
    pub author_kind: CommentAuthorKind,
    /// Username or agent name.
    pub author_name: String,
    /// Author-assigned severity.
    pub severity: CommentSeverity,
    /// Agent run that produced it; `None` for human comments.
    pub run_id: Option<String>,
    /// Free-form category; see [`LocalComment::tag`].
    #[serde(default)]
    pub tag: Option<String>,
    /// Root comment this replies to; `None` starts a new thread.
    #[serde(default)]
    pub parent_id: Option<String>,
    /// GitHub review-comment id the thread discusses, if started from one.
    #[serde(default)]
    pub github_comment_id: Option<u64>,
}

/// One superseded version of a comment body. The current text lives on
/// the comment itself; this is what it used to say.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommentRevision {
    /// Revision id (UUID string).
    pub id: String,
    /// [`LocalComment::id`] this revision belongs to.
    pub comment_id: String,
    /// When this version was replaced.
    pub edited_at: DateTime<Utc>,
    /// Who wrote this version — the agent for the text it generated,
    /// "you" for a version you had edited into place.
    pub editor: String,
    /// The superseded body text.
    pub body: String,
}

/// The PR-level write-up an agent produces alongside its inline
/// comments. One per pull request: a re-review is handed the existing
/// text and revises it rather than starting over, so this is the
/// running account of what agents have found, not a per-run log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrSummary {
    /// Repository of the PR.
    pub repo: RepoRef,
    /// PR number within [`PrSummary::repo`].
    pub pr_number: u64,
    /// Markdown. May link to anchors like `tandem://src/x.rs#L42`.
    pub body: String,
    /// Agent that last wrote it.
    pub agent_name: String,
    /// Run that last wrote it.
    pub run_id: String,
    /// Head SHA the latest revision reviewed — a summary written
    /// against older commits is worth flagging as stale.
    pub head_sha: String,
    /// When the latest revision was written.
    pub updated_at: DateTime<Utc>,
    /// Root of the local comment thread discussing this summary, once
    /// you've replied to it at least once. The summary row is stored as
    /// JSON, so this needed no migration.
    #[serde(default)]
    pub thread_root_id: Option<String>,
}
