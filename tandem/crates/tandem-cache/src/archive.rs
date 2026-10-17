//! Merged-PR archive: when a PR merges, its cached data (detail, diff,
//! comments, agent runs) is kept for three days, then purged at
//! 11:59:59 PM EST on the third day.

use chrono::{DateTime, Duration, FixedOffset, NaiveTime, Offset, TimeZone, Utc};
use rusqlite::params;
use tandem_core::github::{ArchivedPr, PullRequest, RepoRef};
use tandem_core::{Result, TandemError};

use crate::schema::cache_err;
use crate::Cache;

/// EST as a fixed offset (UTC-5), per the retention spec.
fn est() -> FixedOffset {
    // -5h is always a valid offset; the UTC fallback can't actually fire.
    FixedOffset::west_opt(5 * 3600).unwrap_or_else(|| Utc.fix())
}

/// Purge deadline: 11:59:59 PM EST on the third day after archival.
pub fn purge_deadline(archived_at: DateTime<Utc>) -> DateTime<Utc> {
    let local = archived_at.with_timezone(&est());
    let eod = NaiveTime::from_hms_opt(23, 59, 59).unwrap_or(NaiveTime::MIN);
    let deadline_local = (local.date_naive() + Duration::days(3)).and_time(eod);
    match est().from_local_datetime(&deadline_local).single() {
        Some(dt) => dt.with_timezone(&Utc),
        // Fixed offsets never produce ambiguous local times.
        None => archived_at + Duration::days(3),
    }
}

/// What `purge_expired` deleted from the DB, for the caller to mirror
/// on disk: agent-run log directories (by their log file's path) and
/// the (repo, PR number) pairs whose on-disk agent checkout
/// (`ensure_pr_checkout`'s clone) should be removed too.
#[derive(Debug, Default)]
pub struct PurgedRuns {
    /// `log_path` of every deleted run; the run dir is its parent.
    pub log_paths: Vec<String>,
    /// `(repo slug, PR number)` of every purged PR.
    pub repos: Vec<(String, u64)>,
}

/// Every table keyed by a single PR. Used both by the retention sweep
/// and by `forget_pr`, so neither can drift and leave rows behind.
const PR_SCOPED_DELETES: &[&str] = &[
    "DELETE FROM pull_requests WHERE repo = ?1 AND number = ?2",
    "DELETE FROM pr_details WHERE repo = ?1 AND number = ?2",
    "DELETE FROM diffs WHERE repo = ?1 AND number = ?2",
    // Before local_comments: the subquery needs those rows to exist.
    "DELETE FROM comment_revisions WHERE comment_id IN
       (SELECT id FROM local_comments WHERE repo = ?1 AND pr_number = ?2)",
    "DELETE FROM local_comments WHERE repo = ?1 AND pr_number = ?2",
    "DELETE FROM agent_runs WHERE repo = ?1 AND pr_number = ?2",
    "DELETE FROM pr_summaries WHERE repo = ?1 AND number = ?2",
    "DELETE FROM archived_prs WHERE repo = ?1 AND number = ?2",
];

/// Log paths of the runs belonging to one PR, so the caller can delete
/// their directories off disk after the rows are gone.
fn run_log_paths(conn: &rusqlite::Connection, repo: &str, number: u64) -> Result<Vec<String>> {
    let mut stmt = conn
        .prepare("SELECT json FROM agent_runs WHERE repo = ?1 AND pr_number = ?2")
        .map_err(cache_err)?;
    let rows = stmt
        .query_map(params![repo, number], |row| row.get::<_, String>(0))
        .map_err(cache_err)?;
    let mut paths = Vec::new();
    for row in rows {
        let run: tandem_core::agent::AgentRun = serde_json::from_str(&row.map_err(cache_err)?)?;
        paths.push(run.log_path);
    }
    Ok(paths)
}

/// Retention of merged PRs: archive on merge, purge after the deadline.
pub trait ArchiveStore {
    /// Record a merged PR in the archive (no-op if already archived).
    fn archive_pr(&self, pr: &PullRequest, now: DateTime<Utc>) -> Result<()>;
    /// Archived PRs for a repo, most recently archived first.
    fn list_archived(&self, repo: &RepoRef) -> Result<Vec<ArchivedPr>>;
    /// Delete every trace of archive entries whose deadline has passed.
    fn purge_expired(&mut self, now: DateTime<Utc>) -> Result<PurgedRuns>;
    /// Delete everything cached for one PR right now, without waiting
    /// for the retention window — "I'm done with this one, forget it".
    fn forget_pr(&mut self, repo: &RepoRef, number: u64) -> Result<PurgedRuns>;
}

impl ArchiveStore for Cache {
    fn archive_pr(&self, pr: &PullRequest, now: DateTime<Utc>) -> Result<()> {
        self.conn
            .execute(
                "INSERT OR IGNORE INTO archived_prs (repo, number, archived_at, purge_after, json)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    pr.repo.slug(),
                    pr.number,
                    now.to_rfc3339(),
                    purge_deadline(now).to_rfc3339(),
                    serde_json::to_string(pr)?
                ],
            )
            .map_err(cache_err)?;
        Ok(())
    }

    fn list_archived(&self, repo: &RepoRef) -> Result<Vec<ArchivedPr>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT json, archived_at, purge_after FROM archived_prs
                 WHERE repo = ?1 ORDER BY archived_at DESC",
            )
            .map_err(cache_err)?;
        let rows = stmt
            .query_map(params![repo.slug()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(cache_err)?;
        let mut out = Vec::new();
        for row in rows {
            let (json, archived_at, purge_after) = row.map_err(cache_err)?;
            out.push(ArchivedPr {
                pull_request: serde_json::from_str(&json)?,
                archived_at: parse_ts(&archived_at)?,
                purge_after: parse_ts(&purge_after)?,
            });
        }
        Ok(out)
    }

    fn purge_expired(&mut self, now: DateTime<Utc>) -> Result<PurgedRuns> {
        let expired: Vec<(String, u64)> = {
            let mut stmt = self
                .conn
                .prepare("SELECT repo, number FROM archived_prs WHERE purge_after < ?1")
                .map_err(cache_err)?;
            let rows = stmt
                .query_map(params![now.to_rfc3339()], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, u64>(1)?))
                })
                .map_err(cache_err)?;
            rows.collect::<std::result::Result<_, _>>()
                .map_err(cache_err)?
        };

        let mut log_paths = Vec::new();
        for (repo, number) in &expired {
            log_paths.extend(run_log_paths(&self.conn, repo, *number)?);
        }

        let tx = self.conn.transaction().map_err(cache_err)?;
        for (repo, number) in &expired {
            for sql in PR_SCOPED_DELETES {
                tx.execute(sql, params![repo, number]).map_err(cache_err)?;
            }
            tracing::info!(repo, number, "purged archived PR data");
        }
        tx.commit().map_err(cache_err)?;
        Ok(PurgedRuns {
            log_paths,
            repos: expired,
        })
    }

    fn forget_pr(&mut self, repo: &RepoRef, number: u64) -> Result<PurgedRuns> {
        let slug = repo.slug();
        let log_paths = run_log_paths(&self.conn, &slug, number)?;
        let tx = self.conn.transaction().map_err(cache_err)?;
        for sql in PR_SCOPED_DELETES {
            tx.execute(sql, params![slug, number]).map_err(cache_err)?;
        }
        tx.commit().map_err(cache_err)?;
        tracing::info!(repo = %slug, number, "forgot PR data");
        Ok(PurgedRuns {
            log_paths,
            repos: vec![(slug, number)],
        })
    }
}

pub(crate) fn parse_ts(s: &str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| TandemError::Cache(format!("bad timestamp in archive: {e}")))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn deadline_is_est_eod_three_days_out() {
        // Archived 2026-09-16 10:00 EST (15:00 UTC) → purge 2026-09-19
        // 23:59:59 EST = 2026-09-20 04:59:59 UTC.
        let archived = Utc.with_ymd_and_hms(2026, 9, 16, 15, 0, 0).unwrap();
        let deadline = purge_deadline(archived);
        assert_eq!(
            deadline,
            Utc.with_ymd_and_hms(2026, 9, 20, 4, 59, 59).unwrap()
        );
    }

    #[test]
    fn deadline_handles_est_date_rollover() {
        // 2026-09-16 23:30 EST is 2026-09-17 04:30 UTC — the EST date is
        // still the 16th, so purge lands on the 19th EOD EST.
        let archived = Utc.with_ymd_and_hms(2026, 9, 17, 4, 30, 0).unwrap();
        let deadline = purge_deadline(archived);
        assert_eq!(
            deadline,
            Utc.with_ymd_and_hms(2026, 9, 20, 4, 59, 59).unwrap()
        );
    }

    #[test]
    fn purge_expired_reports_repo_and_log_path_for_checkout_cleanup() {
        use crate::ReviewStore;
        use tandem_core::agent::{AgentRun, RunStatus};
        use tandem_core::github::{PrState, User};

        let mut cache = Cache::open_in_memory().unwrap();
        let repo = RepoRef::parse("o/r").unwrap();
        let pr = PullRequest {
            repo: repo.clone(),
            number: 7,
            title: "PR 7".into(),
            body: String::new(),
            state: PrState::Merged,
            draft: false,
            author: User {
                login: "matt".into(),
                avatar_url: None,
            },
            head_ref: "feat".into(),
            head_sha: "abc123".into(),
            base_ref: "main".into(),
            additions: 1,
            deletions: 2,
            changed_files: 1,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            labels: vec![],
            mergeable_state: None,
            requested_reviewers: vec![],
            node_id: None,
            review_decision: None,
            checks_state: None,
            unresolved_threads: 0,
        };
        // Archived well past its 3-day deadline, so it's due for purge.
        cache
            .archive_pr(&pr, Utc::now() - Duration::days(10))
            .unwrap();
        cache
            .put_agent_run(&AgentRun {
                run_id: "run1".into(),
                agent_name: "claude".into(),
                repo_slug: repo.slug(),
                pr_number: pr.number,
                head_sha: "abc123".into(),
                status: RunStatus::Succeeded,
                started_at: Utc::now(),
                finished_at: Some(Utc::now()),
                log_path: "/tmp/tandem-test-run1/events.jsonl".into(),
                comment_count: 0,
                purpose: "pr review".into(),
                target_comment_id: None,
                error: None,
            })
            .unwrap();

        let purged = cache.purge_expired(Utc::now()).unwrap();
        assert_eq!(purged.repos, vec![(repo.slug(), pr.number)]);
        assert_eq!(
            purged.log_paths,
            vec!["/tmp/tandem-test-run1/events.jsonl".to_owned()]
        );
    }
}
