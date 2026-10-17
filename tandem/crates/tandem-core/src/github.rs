//! GitHub-facing domain types, normalized from the REST API.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// `owner/name` reference to a repository.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RepoRef {
    /// Account or organization login that owns the repository.
    pub owner: String,
    /// Repository name without the owner prefix.
    pub name: String,
}

impl RepoRef {
    /// Parse `owner/name`. Returns `None` for anything else.
    pub fn parse(s: &str) -> Option<Self> {
        let (owner, name) = s.split_once('/')?;
        if owner.is_empty() || name.is_empty() || name.contains('/') {
            return None;
        }
        Some(Self {
            owner: owner.to_owned(),
            name: name.to_owned(),
        })
    }

    /// The canonical `owner/name` form — inverse of [`RepoRef::parse`] and the
    /// key used for caches, settings, and `gh` invocations.
    pub fn slug(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }
}

/// A GitHub account as shown next to PRs, reviews, and comments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    /// GitHub login (handle), without the leading `@`.
    pub login: String,
    /// Avatar image URL; `None` when the API omitted it (e.g. ghost users).
    pub avatar_url: Option<String>,
}

/// Lifecycle state of a pull request. GitHub reports merged PRs as
/// `closed`; the client folds in `merged_at` to produce [`PrState::Merged`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrState {
    /// Open for review (drafts included — see [`PullRequest::draft`]).
    Open,
    /// Closed without merging.
    Closed,
    /// Closed by a merge.
    Merged,
}

/// A pull request, normalized from the REST/GraphQL payloads.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequest {
    /// Repository the PR belongs to (its base repo).
    pub repo: RepoRef,
    /// PR number, unique within [`PullRequest::repo`].
    pub number: u64,
    /// Title as currently shown on GitHub.
    pub title: String,
    /// Markdown description; empty when the PR has none.
    pub body: String,
    /// Open / closed / merged.
    pub state: PrState,
    /// True for draft PRs (not yet ready for review).
    pub draft: bool,
    /// Who opened the PR.
    pub author: User,
    /// Branch name the changes come from.
    pub head_ref: String,
    /// Commit SHA at the tip of the head branch — anchors review comments and
    /// cache freshness.
    pub head_sha: String,
    /// Branch the PR merges into.
    pub base_ref: String,
    /// Lines added across the whole PR.
    pub additions: u64,
    /// Lines removed across the whole PR.
    pub deletions: u64,
    /// Number of files touched.
    pub changed_files: u64,
    /// When the PR was opened.
    pub created_at: DateTime<Utc>,
    /// Last activity on the PR (push, comment, label, ...) — drives sort and
    /// cache invalidation.
    pub updated_at: DateTime<Utc>,
    /// Label names, in GitHub's order.
    pub labels: Vec<String>,
    /// GitHub's merge-readiness verdict for the detail view: clean,
    /// blocked, dirty (conflicts), behind, unstable, draft, ...
    #[serde(default)]
    pub mergeable_state: Option<String>,
    /// Logins whose review is still requested (they haven't reviewed).
    #[serde(default)]
    pub requested_reviewers: Vec<String>,
    /// GraphQL node id — needed for auto-merge mutations.
    #[serde(default)]
    pub node_id: Option<String>,
    /// GraphQL reviewDecision: APPROVED / `REVIEW_REQUIRED` /
    /// `CHANGES_REQUESTED` (None when the repo requires no reviews).
    #[serde(default)]
    pub review_decision: Option<String>,
    /// Status-check rollup for the head commit: SUCCESS / FAILURE /
    /// PENDING / ERROR / EXPECTED.
    #[serde(default)]
    pub checks_state: Option<String>,
    /// Count of unresolved review threads (drives the orange edge).
    #[serde(default)]
    pub unresolved_threads: u64,
}

/// Combined commit status / check-run conclusion, normalized.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckState {
    /// Queued or still running.
    Pending,
    /// Passed.
    Success,
    /// Failed, errored, or timed out.
    Failure,
    /// Finished without a pass/fail verdict.
    Neutral,
    /// Cancelled before finishing.
    Cancelled,
    /// Skipped (e.g. filtered out by path rules).
    Skipped,
}

/// One CI check (check run or commit status) on the PR's head commit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckRun {
    /// Check or status context name as shown on GitHub.
    pub name: String,
    /// Normalized conclusion; [`CheckState::Pending`] while still running.
    pub state: CheckState,
    /// Link to the CI provider's page for this check, when GitHub has one.
    pub details_url: Option<String>,
}

/// Verdict of a single submitted GitHub review.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewVerdict {
    /// Approved the PR.
    Approved,
    /// Requested changes before merging.
    ChangesRequested,
    /// Left comments without a verdict.
    Commented,
    /// An earlier verdict that was dismissed.
    Dismissed,
    /// Started but not yet submitted (only visible to its author).
    Pending,
}

/// A review submitted on GitHub (approval, change request, …).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubReview {
    /// GitHub review id; `0` for reviews cached before ids were stored.
    #[serde(default)]
    pub id: u64,
    /// Who submitted the review.
    pub author: User,
    /// What the review concluded.
    pub verdict: ReviewVerdict,
    /// Top-level review body (often empty for plain approvals).
    pub body: String,
    /// Submission time; `None` for pending reviews.
    pub submitted_at: Option<DateTime<Utc>>,
}

/// An issue or review comment that already exists on GitHub (read-only in
/// Tandem — we never post back).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubComment {
    /// GitHub comment id (REST `id`, a.k.a. GraphQL `databaseId`).
    pub id: u64,
    /// Who wrote the comment.
    pub author: User,
    /// Markdown body.
    pub body: String,
    /// File the comment is anchored to; `None` for conversation (issue)
    /// comments.
    pub path: Option<String>,
    /// Current diff line on the new side; `None` for file-less comments or
    /// once the thread is outdated.
    pub line: Option<u64>,
    /// Anchor line at the time the comment was written — GitHub nulls
    /// `line` once the diff moves on (outdated/resolved threads).
    #[serde(default)]
    pub original_line: Option<u64>,
    /// Review comment this replies to (threads).
    #[serde(default)]
    pub in_reply_to_id: Option<u64>,
    /// When the comment was posted.
    pub created_at: DateTime<Utc>,
}

/// A merged PR held in the archive before its data is purged.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchivedPr {
    /// The PR as it was when it merged.
    pub pull_request: PullRequest,
    /// When Tandem moved the PR into the archive.
    pub archived_at: DateTime<Utc>,
    /// Hard purge deadline: 11:59:59 PM EST on the third day after
    /// archival. All cached data for the PR is deleted after this.
    pub purge_after: DateTime<Utc>,
}

/// One GitHub review thread's resolution state, keyed by its root
/// review-comment id (REST databaseId).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewThreadMeta {
    /// GraphQL thread node id — needed for the resolve mutation.
    pub id: String,
    /// True once someone resolved the thread on GitHub.
    pub resolved: bool,
    /// REST id of the first comment in the thread (matches
    /// [`GithubComment::id`]).
    pub root_comment_id: u64,
}

/// Everything Tandem knows about one PR, bundled for the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrDetail {
    /// The PR itself.
    pub pull_request: PullRequest,
    /// CI check runs on the head commit.
    pub checks: Vec<CheckRun>,
    /// Effective per-reviewer verdicts (deduped) — drives the pills.
    pub reviews: Vec<GithubReview>,
    /// Conversation and review comments, in API order.
    pub comments: Vec<GithubComment>,
    /// Raw review events that carried body text — mentions often hide
    /// in these, so the UI lists them alongside comments.
    #[serde(default)]
    pub review_bodies: Vec<GithubReview>,
    /// Review-thread resolution states (drives collapse + resolve).
    #[serde(default)]
    pub review_threads: Vec<ReviewThreadMeta>,
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::RepoRef;

    #[test]
    fn parses_owner_name() {
        let r = RepoRef::parse("modem-dev/hunk").unwrap();
        assert_eq!(r.owner, "modem-dev");
        assert_eq!(r.name, "hunk");
        assert_eq!(r.slug(), "modem-dev/hunk");
    }

    #[test]
    fn rejects_bad_slugs() {
        assert!(RepoRef::parse("nope").is_none());
        assert!(RepoRef::parse("/name").is_none());
        assert!(RepoRef::parse("owner/").is_none());
        assert!(RepoRef::parse("a/b/c").is_none());
    }
}
