//! The review queue as JSON, for readers outside the TUI (`beagle serve`
//! answers the phone app with it). Read-only: nothing here writes, and the
//! store is never created.
//!
//! The rows are the review TUI's own (`state::rows`), the same ones
//! `beagle tandem list` prints, so every view of the queue agrees.

use std::path::PathBuf;

use serde_json::{json, Value};
use tandem_cache::{Cache, ReviewStore, SummaryStore};
use tandem_core::github::RepoRef;
use tandem_core::review::LocalComment;
use tandem_core::{Result, TandemError};

use crate::state::{self, Row};
use crate::verdict::Verdict;

/// Where the review TUI keeps its store (`$TANDEM_TUI_DB`, else the data
/// directory).
pub fn store_path() -> Result<PathBuf> {
    crate::config::db_path()
}

/// The store at `path` opened read-only, or `None` when it does not exist
/// yet (it is never created or migrated here).
pub fn open(path: &std::path::Path) -> Result<Option<Cache>> {
    if !path.is_file() {
        return Ok(None);
    }
    Cache::open_read_only(path).map(Some)
}

fn timestamp(t: chrono::DateTime<chrono::Utc>) -> String {
    t.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

fn row(r: &Row) -> Value {
    json!({
        "repo": r.pr.repo.slug(),
        "number": r.pr.number,
        "title": r.title,
        "stage": r.stage.label(),
        "verdict": r.verdict.map(Verdict::key),
        "open_comments": r.open_comments,
        "requested_at": timestamp(r.requested_at),
        "started_at": r.started_at.map(timestamp),
        "approved": r.approved,
        "error": r.error,
    })
}

/// Every PR in the queue, in the order `beagle tandem list` prints them.
pub fn queue(cache: &Cache) -> Result<Value> {
    let rows: Vec<Value> = state::rows(cache)?.iter().map(row).collect();
    Ok(json!({ "prs": rows }))
}

/// One comment, with its replies when it is a root.
fn comment(c: &LocalComment, replies: &[Value]) -> Value {
    json!({
        "id": c.id,
        "path": c.path,
        "line": c.line,
        "end_line": c.end_line,
        "severity": c.severity,
        "status": c.status,
        "posted": c.posted_github_id.is_some(),
        "author": c.author_name,
        "tag": c.tag,
        "body": c.body,
        "suggestion": c.suggestion,
        "created_at": timestamp(c.created_at),
        "replies": replies,
    })
}

/// One PR's summary, verdict and local comments (roots oldest first,
/// replies nested under them). `repo` is `owner/name`.
pub fn pr(cache: &Cache, repo: &str, number: u64) -> Result<Value> {
    let repo_ref = RepoRef::parse(repo)
        .ok_or_else(|| TandemError::Config(format!("not a repo slug: {repo} (want owner/name)")))?;
    let detail = cache.get_pr_detail(&repo_ref, number)?;
    let summary = cache.get_pr_summary(&repo_ref, number)?;
    let comments = cache.list_comments(&repo_ref, number)?;
    let roots: Vec<Value> = state::roots(&comments)
        .map(|root| {
            let replies = comments
                .iter()
                .filter(|c| c.parent_id.as_deref() == Some(root.id.as_str()))
                .map(|c| comment(c, &[]))
                .collect::<Vec<_>>();
            comment(root, &replies)
        })
        .collect();
    let pull = detail.as_ref().map(|d| &d.pull_request);
    Ok(json!({
        "repo": repo_ref.slug(),
        "number": number,
        "title": pull.map(|p| p.title.as_str()),
        "url": pull.map(|_| format!("https://github.com/{}/pull/{number}", repo_ref.slug())),
        "state": pull.map(|p| p.state),
        "author": pull.map(|p| p.author.login.as_str()),
        "verdict": summary.as_ref().and_then(|s| Verdict::from_summary(&s.body)).map(Verdict::key),
        "summary": summary.as_ref().map(|s| json!({
            "markdown": s.body,
            "agent": s.agent_name,
            "head_sha": s.head_sha,
            "updated_at": timestamp(s.updated_at),
        })),
        "comments": roots,
    }))
}

#[cfg(test)]
#[path = "read_tests.rs"]
mod tests;
