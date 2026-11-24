//! What the review TUI needs beyond [`crate::ReviewQueue`]: every job
//! (not only pending ones) so it can show where each PR is, an atomic
//! claim so two agentd ticks never review the same job, and marks for
//! the PRs you have gone through.

use chrono::{DateTime, Utc};
use rusqlite::params;
use tandem_core::Result;

use crate::archive::parse_ts;
use crate::schema::cache_err;
use crate::{Cache, ReviewJob};

/// One row of `review_jobs`, started or not.
#[derive(Debug, Clone)]
pub struct JobRow {
    /// Job id (a v4 UUID).
    pub id: String,
    /// `owner/name` slug.
    pub repo: String,
    /// PR number.
    pub number: u64,
    /// Who asked, for the record.
    pub requested_by: String,
    /// When the request was recorded.
    pub requested_at: DateTime<Utc>,
    /// Set when an agent claimed the job.
    pub started_at: Option<DateTime<Utc>>,
    /// The agent run reviewing it, once claimed.
    pub run_id: Option<String>,
    /// Why the review failed, if it did.
    pub error: Option<String>,
}

/// Your marks on a PR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrMark {
    /// `owner/name` slug.
    pub repo: String,
    /// PR number.
    pub number: u64,
    /// When you marked it gone-through; None once unmarked.
    pub reviewed_at: Option<DateTime<Utc>>,
    /// When you approved it from the TUI.
    pub approved_at: Option<DateTime<Utc>>,
}

/// Job listing, claiming and triage marks for the review TUI.
pub trait ReviewTriage {
    /// Every job, newest request first.
    fn list_review_jobs(&self) -> Result<Vec<JobRow>>;
    /// Take the oldest pending job, if any. A job is claimed once: a
    /// second claimer racing for it gets the next one, or None.
    fn claim_review_job(&self) -> Result<Option<ReviewJob>>;
    /// Mark a PR gone-through (`true`) or clear the mark (`false`).
    fn set_reviewed(&self, repo: &str, number: u64, reviewed: bool) -> Result<()>;
    /// Record that you approved a PR from the TUI; also marks it
    /// gone-through if it was not already.
    fn set_approved(&self, repo: &str, number: u64) -> Result<()>;
    /// Every PR you have marked, in no particular order.
    fn list_marks(&self) -> Result<Vec<PrMark>>;
}

fn ts(raw: Option<&str>) -> Result<Option<DateTime<Utc>>> {
    raw.map(parse_ts).transpose()
}

impl ReviewTriage for Cache {
    fn list_review_jobs(&self) -> Result<Vec<JobRow>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, repo, number, requested_by, requested_at, started_at, run_id, error
                 FROM review_jobs ORDER BY requested_at DESC",
            )
            .map_err(cache_err)?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, u64>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<String>>(7)?,
                ))
            })
            .map_err(cache_err)?;
        let mut out = Vec::new();
        for row in rows {
            let (id, repo, number, requested_by, requested_at, started_at, run_id, error) =
                row.map_err(cache_err)?;
            out.push(JobRow {
                id,
                repo,
                number,
                requested_by,
                requested_at: parse_ts(&requested_at)?,
                started_at: ts(started_at.as_deref())?,
                run_id,
                error,
            });
        }
        Ok(out)
    }

    fn claim_review_job(&self) -> Result<Option<ReviewJob>> {
        use crate::ReviewQueue as _;
        for job in self.pending_review_jobs()? {
            // The conditional update is the lock: only one claimer sees
            // a row change.
            let changed = self
                .conn
                .execute(
                    "UPDATE review_jobs SET started_at = ?2 WHERE id = ?1 AND started_at IS NULL",
                    params![job.id, Utc::now().to_rfc3339()],
                )
                .map_err(cache_err)?;
            if changed == 1 {
                return Ok(Some(job));
            }
        }
        Ok(None)
    }

    fn set_reviewed(&self, repo: &str, number: u64, reviewed: bool) -> Result<()> {
        let at = reviewed.then(|| Utc::now().to_rfc3339());
        self.conn
            .execute(
                "INSERT INTO pr_marks (repo, number, reviewed_at) VALUES (?1, ?2, ?3)
                 ON CONFLICT(repo, number) DO UPDATE SET reviewed_at = excluded.reviewed_at",
                params![repo, number, at],
            )
            .map_err(cache_err)?;
        Ok(())
    }

    fn set_approved(&self, repo: &str, number: u64) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        self.conn
            .execute(
                "INSERT INTO pr_marks (repo, number, reviewed_at, approved_at) VALUES (?1, ?2, ?3, ?3)
                 ON CONFLICT(repo, number) DO UPDATE SET
                   approved_at = excluded.approved_at,
                   reviewed_at = COALESCE(pr_marks.reviewed_at, excluded.reviewed_at)",
                params![repo, number, now],
            )
            .map_err(cache_err)?;
        Ok(())
    }

    fn list_marks(&self) -> Result<Vec<PrMark>> {
        let mut stmt = self
            .conn
            .prepare("SELECT repo, number, reviewed_at, approved_at FROM pr_marks")
            .map_err(cache_err)?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, u64>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            })
            .map_err(cache_err)?;
        let mut out = Vec::new();
        for row in rows {
            let (repo, number, reviewed_at, approved_at) = row.map_err(cache_err)?;
            out.push(PrMark {
                repo,
                number,
                reviewed_at: ts(reviewed_at.as_deref())?,
                approved_at: ts(approved_at.as_deref())?,
            });
        }
        Ok(out)
    }
}

#[cfg(test)]
// Tests fail loudly on a broken fixture rather than threading Results.
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::ReviewQueue;

    #[test]
    fn a_job_is_claimed_once_oldest_first() {
        let cache = Cache::open_in_memory().unwrap();
        cache.enqueue_review("o/r", 1, None, "tui").unwrap();
        cache.enqueue_review("o/r", 2, None, "agentd").unwrap();

        assert_eq!(cache.claim_review_job().unwrap().unwrap().number, 1);
        assert_eq!(cache.claim_review_job().unwrap().unwrap().number, 2);
        assert!(cache.claim_review_job().unwrap().is_none());

        let jobs = cache.list_review_jobs().unwrap();
        assert_eq!(jobs.len(), 2);
        assert!(jobs.iter().all(|j| j.started_at.is_some()));
    }

    #[test]
    fn marks_toggle_and_approval_implies_reviewed() {
        let cache = Cache::open_in_memory().unwrap();
        cache.set_reviewed("o/r", 1, true).unwrap();
        assert!(cache.list_marks().unwrap()[0].reviewed_at.is_some());
        cache.set_reviewed("o/r", 1, false).unwrap();
        assert!(cache.list_marks().unwrap()[0].reviewed_at.is_none());

        cache.set_approved("o/r", 2).unwrap();
        let mark = cache
            .list_marks()
            .unwrap()
            .into_iter()
            .find(|m| m.number == 2)
            .unwrap();
        assert!(mark.reviewed_at.is_some() && mark.approved_at.is_some());
    }
}
