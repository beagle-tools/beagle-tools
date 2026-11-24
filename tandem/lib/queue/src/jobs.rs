//! The queue's life cycle, driven by the CLI (and `add` from the TUI):
//!
//! ```text
//! add ──▶ review_jobs (pending) ──claim──▶ agent run (running) + context.md
//!                                               │
//!                     ingest (comments, verdict, summary) ──▶ succeeded
//!                     fail (gate or reviewer gave up)     ──▶ failed
//! ```

use std::path::{Path, PathBuf};

use chrono::Utc;
use tandem_cache::{Cache, ReviewQueue, ReviewStore, ReviewTriage, SummaryStore};
use tandem_core::agent::{AgentRun, RunStatus};
use tandem_core::github::RepoRef;
use tandem_core::review::{CommentAuthorKind, CommentSeverity, NewLocalComment, PrSummary};
use tandem_core::{Result, TandemError};
use tandem_github::GithubClient;

use crate::context::{self, ContextInput};
use crate::gh;
use crate::ingest::{self, AgentLine};
use crate::pr_ref::PrRef;
use crate::verdict::Verdict;

/// The `purpose` every run this tool records carries.
pub const PURPOSE: &str = "pr review";

/// Queues `pr` for review. Returns the job id. A PR you had gone
/// through comes back unmarked: the new review is something to read.
pub fn add(cache: &Cache, pr: &PrRef, by: &str) -> Result<String> {
    let id = cache.enqueue_review(&pr.repo.slug(), pr.number, None, by)?;
    cache.set_reviewed(&pr.repo.slug(), pr.number, false)?;
    Ok(id)
}

/// A job handed to a reviewer.
#[derive(Debug, Clone)]
pub struct Claimed {
    pub run_id: String,
    pub pr: PrRef,
    pub context: PathBuf,
}

/// Claims the oldest pending job, fetches its PR, and writes
/// `<out>/context.md` for the reviewer. A job whose PR cannot be fetched
/// is failed (the TUI shows why) and the next one is tried.
pub async fn claim(
    cache: &Cache,
    client: &GithubClient,
    out: &Path,
    agent: &str,
    program: &str,
) -> Result<Option<Claimed>> {
    while let Some(job) = cache.claim_review_job()? {
        let run_id = uuid::Uuid::new_v4().to_string();
        let Some(repo) = RepoRef::parse(&job.repo) else {
            cache.finish_review_job(&job.id, None, Some("not a repo slug"))?;
            continue;
        };
        let pr = PrRef {
            repo,
            number: job.number,
        };
        let mut run = AgentRun {
            run_id: run_id.clone(),
            agent_name: agent.to_owned(),
            repo_slug: pr.repo.slug(),
            pr_number: pr.number,
            head_sha: String::new(),
            status: RunStatus::Running,
            started_at: Utc::now(),
            finished_at: None,
            log_path: out.display().to_string(),
            comment_count: 0,
            purpose: PURPOSE.to_owned(),
            target_comment_id: None,
            error: None,
        };
        match prepare(cache, client, &pr, &run_id, out, program).await {
            Ok((context, head_sha)) => {
                run.head_sha = head_sha;
                cache.put_agent_run(&run)?;
                cache.finish_review_job(&job.id, Some(&run_id), None)?;
                return Ok(Some(Claimed {
                    run_id,
                    pr,
                    context,
                }));
            }
            Err(e) => {
                let error = format!("could not fetch {}: {e}", pr.label());
                eprintln!("{error}");
                run.status = RunStatus::Failed;
                run.finished_at = Some(Utc::now());
                run.error = Some(error.clone());
                cache.put_agent_run(&run)?;
                cache.finish_review_job(&job.id, Some(&run_id), Some(&error))?;
            }
        }
    }
    Ok(None)
}

async fn prepare(
    cache: &Cache,
    client: &GithubClient,
    pr: &PrRef,
    run_id: &str,
    out: &Path,
    program: &str,
) -> Result<(PathBuf, String)> {
    let detail = gh::fetch(client, cache, pr).await?;
    let head = detail.pull_request.head_sha.clone();
    let raw = cache
        .get_raw_diff(&pr.repo, pr.number, &head)?
        .unwrap_or_default();
    let summary = cache.get_pr_summary(&pr.repo, pr.number)?;
    let text = context::build(&ContextInput {
        run_id,
        program,
        detail: &detail,
        raw_diff: &raw,
        existing_summary: summary.as_ref().map(|s| s.body.as_str()),
    });
    std::fs::create_dir_all(out)?;
    let path = out.join("context.md");
    std::fs::write(&path, text)?;
    Ok((path, head))
}

/// Why `ingest` refused: the run is not one it can load into, or the
/// review has problems the agent must fix.
#[derive(Debug)]
pub enum IngestError {
    Run(String),
    Review(Vec<String>),
    Store(TandemError),
}

impl From<TandemError> for IngestError {
    fn from(e: TandemError) -> Self {
        Self::Store(e)
    }
}

/// Loads `text` (the agent's lines) into the store for `run_id`.
/// Returns how many comments were stored.
pub fn ingest(cache: &Cache, run_id: &str, text: &str) -> std::result::Result<usize, IngestError> {
    let Some(mut run) = cache.get_agent_run(run_id)? else {
        return Err(IngestError::Run(format!(
            "no run {run_id} — use the id from claim"
        )));
    };
    if run.status != RunStatus::Running {
        return Err(IngestError::Run(format!(
            "run {run_id} is already {:?}; a review loads once",
            run.status
        )));
    }
    let repo = RepoRef::parse(&run.repo_slug)
        .ok_or_else(|| IngestError::Run(format!("bad repo on run: {}", run.repo_slug)))?;
    let diff = cache
        .get_diff(&repo, run.pr_number, &run.head_sha)?
        .ok_or_else(|| IngestError::Run("the run's diff is not in the store".into()))?;
    let existing = cache.get_pr_summary(&repo, run.pr_number)?;
    let review = ingest::parse(text, &diff, existing.is_some()).map_err(IngestError::Review)?;

    for line in &review.comments {
        store_comment(cache, &run, &repo, line)?;
    }
    let heading = review.verdict.heading(&review.reason);
    let body = match (&review.summary, &existing) {
        (Some(summary), _) => format!("{heading}\n\n{}", summary.trim()),
        (None, Some(old)) => Verdict::replace_heading(&old.body, &heading),
        (None, None) => heading,
    };
    cache.put_pr_summary(&PrSummary {
        repo,
        pr_number: run.pr_number,
        body,
        agent_name: run.agent_name.clone(),
        run_id: run.run_id.clone(),
        head_sha: run.head_sha.clone(),
        updated_at: Utc::now(),
        thread_root_id: existing.and_then(|s| s.thread_root_id),
    })?;
    run.status = RunStatus::Succeeded;
    run.finished_at = Some(Utc::now());
    run.comment_count = review.comments.len() as u64;
    cache.put_agent_run(&run)?;
    Ok(review.comments.len())
}

/// The comment, then its history as a local reply under it.
fn store_comment(cache: &Cache, run: &AgentRun, repo: &RepoRef, line: &AgentLine) -> Result<()> {
    let new = |body: String, severity, tag: Option<String>, parent_id| NewLocalComment {
        repo: repo.clone(),
        pr_number: run.pr_number,
        head_sha: run.head_sha.clone(),
        path: line.path.clone(),
        side: line.side,
        line: line.line,
        end_line: line.end_line,
        body,
        suggestion: None,
        author_kind: CommentAuthorKind::Agent,
        author_name: run.agent_name.clone(),
        severity,
        run_id: Some(run.run_id.clone()),
        tag,
        parent_id,
        github_comment_id: None,
    };
    let mut root = new(line.body.clone(), line.severity, line.tag.clone(), None);
    root.suggestion.clone_from(&line.suggestion);
    let root = cache.add_comment(root)?;
    let history = match &line.duplicate_of {
        Some(of) if !of.trim().is_empty() => {
            format!("**Duplicate of** {}\n\n{}", of.trim(), line.context.trim())
        }
        _ => line.context.trim().to_owned(),
    };
    cache.add_comment(new(
        history,
        CommentSeverity::Info,
        Some("history".to_owned()),
        Some(root.id),
    ))?;
    Ok(())
}

/// Marks `run_id` failed with `error` (a gate or reviewer that gave up).
pub fn fail(cache: &Cache, run_id: &str, error: &str) -> Result<()> {
    let mut run = cache
        .get_agent_run(run_id)?
        .ok_or_else(|| TandemError::Config(format!("no run {run_id}")))?;
    run.status = RunStatus::Failed;
    run.finished_at = Some(Utc::now());
    run.error = Some(error.to_owned());
    cache.put_agent_run(&run)
}

#[cfg(test)]
#[path = "jobs_tests.rs"]
mod tests;
