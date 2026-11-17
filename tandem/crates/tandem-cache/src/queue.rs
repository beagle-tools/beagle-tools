//! The seam another process works through. Tandem is a desktop app
//! with no server, so a cron job can't call it — but both sides can
//! reach the same SQLite file. The outside writes a *request* to review
//! something; Tandem decides when and how, using the agent config it
//! already has. Nothing out there starts an agent itself.

use chrono::{DateTime, Utc};
use rusqlite::params;
use tandem_core::github::{PullRequest, RepoRef};
use tandem_core::{Result, TandemError};
use uuid::Uuid;

use crate::archive::parse_ts;
use crate::schema::cache_err;
use crate::Cache;

/// A review somebody asked for and Tandem hasn't started yet.
#[derive(Debug, Clone)]
pub struct ReviewJob {
    /// Job id (a v4 UUID).
    pub id: String,
    /// `owner/name` slug.
    pub repo: String,
    /// PR number.
    pub number: u64,
    /// `None` means "whichever agent Tandem would use by default".
    pub agent: Option<String>,
    /// Who asked, for the record (e.g. `tandem-cli`, a job name).
    pub requested_by: String,
    /// When the request was recorded.
    pub requested_at: DateTime<Utc>,
}

/// The review-request inbox mirror and the queue of reviews other
/// processes asked for.
pub trait ReviewQueue {
    /// Mirror the review-request inbox into the database so a process
    /// without a GitHub token can still see what's waiting. Replaces the
    /// previous mirror wholesale.
    fn put_review_requests(&self, prs: &[PullRequest]) -> Result<()>;
    /// The mirrored inbox, most recently fetched first.
    fn list_review_requests(&self) -> Result<Vec<PullRequest>>;

    /// Ask Tandem to review a PR. Returns the job id.
    fn enqueue_review(
        &self,
        repo: &str,
        number: u64,
        agent: Option<&str>,
        requested_by: &str,
    ) -> Result<String>;
    /// Jobs not yet started, oldest first.
    fn pending_review_jobs(&self) -> Result<Vec<ReviewJob>>;
    /// Mark a job started (with its run) or failed (with its error).
    fn finish_review_job(&self, id: &str, run_id: Option<&str>, error: Option<&str>) -> Result<()>;
}

impl ReviewQueue for Cache {
    fn put_review_requests(&self, prs: &[PullRequest]) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        // Replace wholesale: a PR that left the inbox is no longer a
        // request, and a stale row would have a cron reviewing things
        // nobody is asking about.
        self.conn
            .execute("DELETE FROM review_requests", [])
            .map_err(cache_err)?;
        for pr in prs {
            self.conn
                .execute(
                    "INSERT OR REPLACE INTO review_requests (repo, number, fetched_at, json)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![pr.repo.slug(), pr.number, now, serde_json::to_string(pr)?],
                )
                .map_err(cache_err)?;
        }
        Ok(())
    }

    fn list_review_requests(&self) -> Result<Vec<PullRequest>> {
        let mut stmt = self
            .conn
            .prepare("SELECT json FROM review_requests ORDER BY fetched_at DESC, repo, number")
            .map_err(cache_err)?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(cache_err)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(serde_json::from_str(&row.map_err(cache_err)?)?);
        }
        Ok(out)
    }

    fn enqueue_review(
        &self,
        repo: &str,
        number: u64,
        agent: Option<&str>,
        requested_by: &str,
    ) -> Result<String> {
        // Parsed here so a malformed slug fails at the door rather than
        // inside a background task nobody is watching.
        if RepoRef::parse(repo).is_none() {
            return Err(TandemError::Config(format!(
                "not a repo slug: {repo} (want owner/name)"
            )));
        }
        let id = Uuid::new_v4().to_string();
        self.conn
            .execute(
                "INSERT INTO review_jobs (id, repo, number, agent, requested_by, requested_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    id,
                    repo,
                    number,
                    agent,
                    requested_by,
                    Utc::now().to_rfc3339()
                ],
            )
            .map_err(cache_err)?;
        Ok(id)
    }

    fn pending_review_jobs(&self) -> Result<Vec<ReviewJob>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, repo, number, agent, requested_by, requested_at FROM review_jobs
                 WHERE started_at IS NULL ORDER BY requested_at ASC",
            )
            .map_err(cache_err)?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, u64>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                ))
            })
            .map_err(cache_err)?;
        let mut out = Vec::new();
        for row in rows {
            let (id, repo, number, agent, requested_by, requested_at) = row.map_err(cache_err)?;
            out.push(ReviewJob {
                id,
                repo,
                number,
                agent,
                requested_by,
                requested_at: parse_ts(&requested_at)?,
            });
        }
        Ok(out)
    }

    fn finish_review_job(&self, id: &str, run_id: Option<&str>, error: Option<&str>) -> Result<()> {
        self.conn
            .execute(
                "UPDATE review_jobs SET started_at = ?2, run_id = ?3, error = ?4 WHERE id = ?1",
                params![id, Utc::now().to_rfc3339(), run_id, error],
            )
            .map_err(cache_err)?;
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn pr(repo: &str, number: u64) -> PullRequest {
        serde_json::from_value(serde_json::json!({
            "repo": RepoRef::parse(repo).unwrap(),
            "number": number,
            "title": "t",
            "body": "",
            "author": { "login": "octo", "avatar_url": null },
            "state": "open",
            "draft": false,
            "head_ref": "h",
            "base_ref": "main",
            "head_sha": "abc",
            "additions": 0,
            "deletions": 0,
            "changed_files": 0,
            "labels": [],
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z",
        }))
        .unwrap()
    }

    #[test]
    fn a_job_is_pending_until_it_is_finished() {
        let cache = Cache::open_in_memory().unwrap();
        let id = cache
            .enqueue_review("o/r", 7, Some("claude"), "agentd")
            .unwrap();

        let pending = cache.pending_review_jobs().unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].number, 7);
        assert_eq!(pending[0].agent.as_deref(), Some("claude"));
        assert_eq!(pending[0].requested_by, "agentd");

        cache.finish_review_job(&id, Some("run-1"), None).unwrap();
        assert!(cache.pending_review_jobs().unwrap().is_empty());
    }

    #[test]
    fn a_bad_repo_is_refused_at_the_door() {
        let cache = Cache::open_in_memory().unwrap();
        assert!(cache
            .enqueue_review("not-a-slug", 1, None, "agentd")
            .is_err());
    }

    #[test]
    fn the_inbox_mirror_is_replaced_wholesale() {
        let cache = Cache::open_in_memory().unwrap();
        cache
            .put_review_requests(&[pr("o/r", 1), pr("o/r", 2)])
            .unwrap();
        cache.put_review_requests(&[pr("o/r", 3)]).unwrap();
        let numbers: Vec<u64> = cache
            .list_review_requests()
            .unwrap()
            .into_iter()
            .map(|p| p.number)
            .collect();
        assert_eq!(numbers, vec![3]);
    }
}
