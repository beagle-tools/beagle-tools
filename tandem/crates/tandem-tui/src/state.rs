//! Where each queued PR stands, read from the store: the newest job, the
//! run that reviewed it, the verdict, and your marks. The TUI list and
//! `beagle tandem list` both show this.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use tandem_cache::{Cache, ReviewStore, ReviewTriage, SummaryStore};
use tandem_core::agent::{AgentRun, RunStatus};
use tandem_core::github::{PrState, RepoRef};
use tandem_core::review::{CommentStatus, LocalComment};
use tandem_core::Result;

use crate::pr_ref::PrRef;
use crate::verdict::Verdict;

/// Where a PR is in the review flow, in the order the list shows them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stage {
    /// Reviewed by the agent, waiting for you.
    Ready,
    Reviewing,
    Queued,
    Failed,
    /// You have gone through it.
    Done,
    /// Merged on GitHub (seen by the background check or a fetch).
    Merged,
    /// Closed on GitHub without merging.
    Closed,
}

impl Stage {
    pub fn label(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Reviewing => "reviewing",
            Self::Queued => "queued",
            Self::Failed => "failed",
            Self::Done => "done",
            Self::Merged => "merged",
            Self::Closed => "closed",
        }
    }

    /// GitHub can no longer change it, so the background check skips it.
    pub fn is_finished(self) -> bool {
        matches!(self, Self::Merged | Self::Closed)
    }
}

/// One PR in the list.
#[derive(Debug, Clone)]
pub struct Row {
    pub pr: PrRef,
    pub title: Option<String>,
    pub stage: Stage,
    pub verdict: Option<Verdict>,
    /// Agent comments not yet posted or dismissed.
    pub open_comments: usize,
    pub requested_at: DateTime<Utc>,
    /// When the current review started (for "reviewing for 4m").
    pub started_at: Option<DateTime<Utc>>,
    pub approved: bool,
    pub error: Option<String>,
}

/// A review still running after this long has died (the agentd reviewer
/// times out at 30 minutes and loads its result before it exits).
const STALLED_AFTER_MINUTES: i64 = 45;

/// Every PR that was ever queued, Ready first, newest request first
/// within a stage.
pub fn rows(cache: &Cache) -> Result<Vec<Row>> {
    let marks: HashMap<(String, u64), _> = cache
        .list_marks()?
        .into_iter()
        .map(|m| ((m.repo.clone(), m.number), m))
        .collect();
    let mut seen = HashSet::new();
    let mut out: Vec<Row> = Vec::new();
    // Jobs come newest first: the first job per PR is its current one.
    for job in cache.list_review_jobs()? {
        if !seen.insert((job.repo.clone(), job.number)) {
            continue;
        }
        let Some(repo) = RepoRef::parse(&job.repo) else {
            continue;
        };
        let run = match &job.run_id {
            Some(id) => cache.get_agent_run(id)?,
            None => None,
        };
        let mark = marks.get(&(job.repo.clone(), job.number));
        let reviewed = mark.is_some_and(|m| m.reviewed_at.is_some());
        let stage = match (&job.started_at, &run, &job.error) {
            (None, _, _) => Stage::Queued,
            (Some(_), _, Some(_)) => Stage::Failed,
            (Some(_), Some(run), None) => match run.status {
                RunStatus::Succeeded if reviewed => Stage::Done,
                RunStatus::Succeeded => Stage::Ready,
                RunStatus::Starting | RunStatus::Running if stalled(run) => Stage::Failed,
                RunStatus::Starting | RunStatus::Running => Stage::Reviewing,
                RunStatus::Failed | RunStatus::Cancelled | RunStatus::TimedOut => Stage::Failed,
            },
            (Some(_), None, None) => Stage::Reviewing,
        };
        let detail = cache.get_pr_detail(&repo, job.number)?;
        // GitHub's word beats the review flow: a merged PR is finished
        // whatever its review stage was.
        let stage = match detail.as_ref().map(|d| d.pull_request.state) {
            Some(PrState::Merged) => Stage::Merged,
            Some(PrState::Closed) => Stage::Closed,
            _ => stage,
        };
        let summary = cache.get_pr_summary(&repo, job.number)?;
        let comments = cache.list_comments(&repo, job.number)?;
        out.push(Row {
            title: detail.map(|d| d.pull_request.title),
            pr: PrRef {
                repo,
                number: job.number,
            },
            stage,
            verdict: summary.and_then(|s| Verdict::from_summary(&s.body)),
            open_comments: roots(&comments)
                .filter(|c| c.status == CommentStatus::Open && c.posted_github_id.is_none())
                .count(),
            requested_at: job.requested_at,
            started_at: run.as_ref().map(|r| r.started_at),
            approved: mark.is_some_and(|m| m.approved_at.is_some()),
            error: job.error.or_else(|| run.and_then(failure)),
        });
    }
    out.sort_by(|a, b| {
        a.stage
            .cmp(&b.stage)
            .then(b.requested_at.cmp(&a.requested_at))
    });
    Ok(out)
}

fn stalled(run: &AgentRun) -> bool {
    (Utc::now() - run.started_at).num_minutes() >= STALLED_AFTER_MINUTES
}

fn failure(run: AgentRun) -> Option<String> {
    let stuck = matches!(run.status, RunStatus::Starting | RunStatus::Running) && stalled(&run);
    if stuck {
        return Some(format!(
            "no review after {STALLED_AFTER_MINUTES} minutes — the agent stopped without loading one (r to try again)"
        ));
    }
    run.error
}

/// The review comments themselves (not their history replies).
pub fn roots(comments: &[LocalComment]) -> impl Iterator<Item = &LocalComment> {
    comments.iter().filter(|c| c.parent_id.is_none())
}

#[cfg(test)]
// Tests panic on a broken fixture; that is the failure mode we want.
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use tandem_cache::ReviewQueue;

    #[test]
    fn stages_follow_the_job_and_your_marks() {
        let cache = Cache::open_in_memory().unwrap();
        cache.enqueue_review("o/r", 1, None, "tui").unwrap();
        assert_eq!(rows(&cache).unwrap()[0].stage, Stage::Queued);

        let job = cache.claim_review_job().unwrap().unwrap();
        assert_eq!(rows(&cache).unwrap()[0].stage, Stage::Reviewing);

        cache
            .put_agent_run(&AgentRun {
                run_id: "r1".into(),
                agent_name: "claude".into(),
                repo_slug: "o/r".into(),
                pr_number: 1,
                head_sha: "s".into(),
                status: RunStatus::Succeeded,
                started_at: Utc::now(),
                finished_at: Some(Utc::now()),
                log_path: String::new(),
                comment_count: 0,
                purpose: "pr review".into(),
                target_comment_id: None,
                error: None,
            })
            .unwrap();
        cache.finish_review_job(&job.id, Some("r1"), None).unwrap();
        assert_eq!(rows(&cache).unwrap()[0].stage, Stage::Ready);

        cache.set_reviewed("o/r", 1, true).unwrap();
        assert_eq!(rows(&cache).unwrap()[0].stage, Stage::Done);

        // Asking again puts it back in the queue, at the top of its PR.
        std::thread::sleep(std::time::Duration::from_millis(5));
        cache.enqueue_review("o/r", 1, None, "tui").unwrap();
        let all = rows(&cache).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].stage, Stage::Queued);
    }

    /// A stored PR in `state`, as the background check would leave it.
    fn store_pr(cache: &Cache, number: u64, state: &str) {
        let detail: tandem_core::github::PrDetail = serde_json::from_value(serde_json::json!({
            "pull_request": {
                "repo": {"owner": "o", "name": "r"}, "number": number, "title": "t",
                "body": "", "state": state, "draft": false,
                "author": {"login": "octocat", "avatar_url": null},
                "head_ref": "f", "head_sha": "s", "base_ref": "main",
                "additions": 0, "deletions": 0, "changed_files": 0,
                "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z",
                "labels": [], "mergeable_state": null, "requested_reviewers": [],
                "node_id": null, "review_decision": null, "checks_state": null,
                "unresolved_threads": 0
            },
            "checks": [], "reviews": [], "comments": []
        }))
        .unwrap();
        let repo = RepoRef::parse("o/r").unwrap();
        cache.put_pr_detail(&repo, &detail).unwrap();
    }

    #[test]
    fn merged_and_closed_prs_are_finished_and_sort_last() {
        let cache = Cache::open_in_memory().unwrap();
        for n in [1, 2, 3] {
            cache.enqueue_review("o/r", n, None, "tui").unwrap();
        }
        store_pr(&cache, 1, "merged");
        store_pr(&cache, 2, "closed");
        store_pr(&cache, 3, "open");
        let all = rows(&cache).unwrap();
        let stage = |n: u64| all.iter().find(|r| r.pr.number == n).unwrap().stage;
        assert_eq!(stage(1), Stage::Merged);
        assert_eq!(stage(2), Stage::Closed);
        assert_eq!(stage(3), Stage::Queued);
        // The open one first; the background check only asks about it.
        assert_eq!(all[0].pr.number, 3);
        assert!(!Stage::Queued.is_finished());
        assert!(Stage::Merged.is_finished() && Stage::Closed.is_finished());
    }
}
