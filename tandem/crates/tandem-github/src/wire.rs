//! Wire-format structs for the GitHub REST API and their conversions
//! into `tandem-core` domain types. Kept separate so `client.rs` stays
//! focused on HTTP.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tandem_core::github::{
    CheckRun, CheckState, GithubComment, GithubReview, PrState, PullRequest, RepoRef,
    ReviewVerdict, User,
};

/// Search API response — we only need the matching PR numbers.
#[derive(Debug, Deserialize)]
pub struct WireSearch {
    #[serde(default)]
    pub items: Vec<WireSearchItem>,
}

#[derive(Debug, Deserialize)]
pub struct WireSearchItem {
    pub number: u64,
    /// e.g. <https://api.github.com/repos/owner/name> — present in global
    /// searches where the repo isn't implied by the query.
    #[serde(default)]
    pub repository_url: Option<String>,
}

/// What merge flows a repository actually allows.
// Four independent repo toggles mirrored to the UI as-is; an enum or
// bitflags would only obscure the IPC shape.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeOptions {
    /// Squash merging is enabled.
    pub squash: bool,
    /// Merge commits are enabled.
    pub merge: bool,
    /// Rebase merging is enabled.
    pub rebase: bool,
    /// Auto-merge can be turned on for PRs (off when unknown).
    pub auto_merge: bool,
}

/// Slim repo row for the settings repo browser.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoSummary {
    /// `owner/name`.
    pub full_name: String,
    /// True for private (or internal) repos.
    pub private: bool,
    /// Repo description, when one is set.
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct WireUser {
    pub login: String,
    pub avatar_url: Option<String>,
}

impl From<WireUser> for User {
    fn from(w: WireUser) -> Self {
        Self {
            login: w.login,
            avatar_url: w.avatar_url,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct WireBranch {
    #[serde(rename = "ref")]
    pub git_ref: String,
    pub sha: String,
}

#[derive(Debug, Deserialize)]
pub struct WireLabel {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct WirePull {
    pub number: u64,
    pub title: String,
    pub body: Option<String>,
    pub state: String,
    pub merged_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub draft: bool,
    pub user: WireUser,
    pub head: WireBranch,
    pub base: WireBranch,
    #[serde(default)]
    pub additions: u64,
    #[serde(default)]
    pub deletions: u64,
    #[serde(default)]
    pub changed_files: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub labels: Vec<WireLabel>,
    #[serde(default)]
    pub mergeable_state: Option<String>,
    #[serde(default)]
    pub requested_reviewers: Vec<WireUser>,
    #[serde(default)]
    pub node_id: Option<String>,
}

impl WirePull {
    pub fn into_domain(self, repo: &RepoRef) -> PullRequest {
        let state = if self.merged_at.is_some() {
            PrState::Merged
        } else if self.state == "closed" {
            PrState::Closed
        } else {
            PrState::Open
        };
        PullRequest {
            repo: repo.clone(),
            number: self.number,
            title: self.title,
            body: self.body.unwrap_or_default(),
            state,
            draft: self.draft,
            author: self.user.into(),
            head_ref: self.head.git_ref,
            head_sha: self.head.sha,
            base_ref: self.base.git_ref,
            additions: self.additions,
            deletions: self.deletions,
            changed_files: self.changed_files,
            created_at: self.created_at,
            updated_at: self.updated_at,
            labels: self.labels.into_iter().map(|l| l.name).collect(),
            mergeable_state: self.mergeable_state,
            requested_reviewers: self
                .requested_reviewers
                .into_iter()
                .map(|u| u.login)
                .collect(),
            node_id: self.node_id,
            review_decision: None,
            checks_state: None,
            unresolved_threads: 0,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct WireCheckRunList {
    #[serde(default)]
    pub check_runs: Vec<WireCheckRun>,
}

#[derive(Debug, Deserialize)]
pub struct WireCheckRun {
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
    pub details_url: Option<String>,
}

impl From<WireCheckRun> for CheckRun {
    fn from(w: WireCheckRun) -> Self {
        let state = match (w.status.as_str(), w.conclusion.as_deref()) {
            (_, Some("success")) => CheckState::Success,
            (_, Some("failure" | "timed_out" | "action_required")) => CheckState::Failure,
            (_, Some("neutral")) => CheckState::Neutral,
            (_, Some("cancelled")) => CheckState::Cancelled,
            (_, Some("skipped")) => CheckState::Skipped,
            _ => CheckState::Pending,
        };
        Self {
            name: w.name,
            state,
            details_url: w.details_url,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct WireReview {
    #[serde(default)]
    pub id: u64,
    pub user: WireUser,
    pub state: String,
    pub body: Option<String>,
    pub submitted_at: Option<DateTime<Utc>>,
}

impl From<WireReview> for GithubReview {
    fn from(w: WireReview) -> Self {
        let verdict = match w.state.as_str() {
            "APPROVED" => ReviewVerdict::Approved,
            "CHANGES_REQUESTED" => ReviewVerdict::ChangesRequested,
            "COMMENTED" => ReviewVerdict::Commented,
            "DISMISSED" => ReviewVerdict::Dismissed,
            _ => ReviewVerdict::Pending,
        };
        Self {
            id: w.id,
            author: w.user.into(),
            verdict,
            body: w.body.unwrap_or_default(),
            submitted_at: w.submitted_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct WireComment {
    pub id: u64,
    pub user: WireUser,
    pub body: Option<String>,
    pub path: Option<String>,
    pub line: Option<u64>,
    #[serde(default)]
    pub original_line: Option<u64>,
    #[serde(default)]
    pub in_reply_to_id: Option<u64>,
    pub created_at: DateTime<Utc>,
}

impl From<WireComment> for GithubComment {
    fn from(w: WireComment) -> Self {
        Self {
            id: w.id,
            author: w.user.into(),
            body: w.body.unwrap_or_default(),
            path: w.path,
            line: w.line,
            original_line: w.original_line,
            in_reply_to_id: w.in_reply_to_id,
            created_at: w.created_at,
        }
    }
}

/// Collapse raw review events into each reviewer's effective verdict:
/// the latest APPROVED / `CHANGES_REQUESTED` per author, cleared by a
/// later DISMISSED. Comment-only reviews never override a verdict.
pub fn effective_reviews(mut reviews: Vec<GithubReview>) -> Vec<GithubReview> {
    reviews.sort_by_key(|r| r.submitted_at);
    let mut latest: Vec<GithubReview> = Vec::new();
    for review in reviews {
        let author = review.author.login.clone();
        match review.verdict {
            ReviewVerdict::Approved | ReviewVerdict::ChangesRequested => {
                latest.retain(|r| r.author.login != author);
                latest.push(review);
            }
            ReviewVerdict::Dismissed => {
                latest.retain(|r| r.author.login != author);
            }
            ReviewVerdict::Commented | ReviewVerdict::Pending => {}
        }
    }
    latest
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn pull_state_normalization() {
        let json = r#"{
            "number": 7, "title": "t", "body": null, "state": "closed",
            "merged_at": "2026-01-02T03:04:05Z", "draft": false,
            "user": {"login": "matt", "avatar_url": null},
            "head": {"ref": "feat", "sha": "abc"},
            "base": {"ref": "main", "sha": "def"},
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-02T00:00:00Z"
        }"#;
        let wire: WirePull = serde_json::from_str(json).unwrap();
        let repo = RepoRef::parse("o/r").unwrap();
        let pr = wire.into_domain(&repo);
        assert_eq!(pr.state, PrState::Merged);
        assert_eq!(pr.body, "");
        assert_eq!(pr.head_sha, "abc");
    }

    #[test]
    fn effective_reviews_dedupe_and_dismiss() {
        let review = |login: &str, verdict: ReviewVerdict, minute: u32| GithubReview {
            id: u64::from(minute),
            author: User {
                login: login.into(),
                avatar_url: None,
            },
            verdict,
            body: String::new(),
            submitted_at: chrono::Utc
                .with_ymd_and_hms(2026, 1, 1, 0, minute, 0)
                .single(),
        };
        let effective = effective_reviews(vec![
            review("matt", ReviewVerdict::Approved, 1),
            review("matt", ReviewVerdict::Approved, 2),
            review("sam", ReviewVerdict::ChangesRequested, 3),
            review("sam", ReviewVerdict::Approved, 4),
            review("kai", ReviewVerdict::Approved, 5),
            review("kai", ReviewVerdict::Dismissed, 6),
            review("lee", ReviewVerdict::Commented, 7),
        ]);
        assert_eq!(effective.len(), 2);
        assert!(effective
            .iter()
            .any(|r| r.author.login == "matt" && r.verdict == ReviewVerdict::Approved));
        assert!(effective
            .iter()
            .any(|r| r.author.login == "sam" && r.verdict == ReviewVerdict::Approved));
    }

    #[test]
    fn check_conclusion_mapping() {
        let run = WireCheckRun {
            name: "ci".into(),
            status: "completed".into(),
            conclusion: Some("timed_out".into()),
            details_url: None,
        };
        assert_eq!(CheckRun::from(run).state, CheckState::Failure);

        let pending = WireCheckRun {
            name: "ci".into(),
            status: "in_progress".into(),
            conclusion: None,
            details_url: None,
        };
        assert_eq!(CheckRun::from(pending).state, CheckState::Pending);
    }
}
