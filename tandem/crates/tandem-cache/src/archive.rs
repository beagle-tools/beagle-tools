//! Merged-PR archive: when a PR merges, its cached data (detail, diff,
//! comments, agent runs) is kept for three days, then purged at
//! 11:59:59 PM US Eastern time (EST or EDT) on the third day.

use chrono::{
    DateTime, Datelike, Duration, FixedOffset, NaiveDate, NaiveTime, Offset, TimeZone, Utc, Weekday,
};
use rusqlite::params;
use tandem_core::github::{ArchivedPr, PullRequest, RepoRef};
use tandem_core::{Result, TandemError};

use crate::schema::cache_err;
use crate::Cache;

/// US Eastern time's offset on `date` at end of day: EDT (UTC-4) from the
/// second Sunday of March to the day before the first Sunday of November,
/// EST (UTC-5) otherwise. The switch happens at 2 AM, so 11:59:59 PM on a
/// switch day already has the new offset.
fn eastern(date: NaiveDate) -> FixedOffset {
    let nth_sunday = |month: u32, n: u8| {
        NaiveDate::from_weekday_of_month_opt(date.year(), month, Weekday::Sun, n)
    };
    let dst = match (nth_sunday(3, 2), nth_sunday(11, 1)) {
        (Some(start), Some(end)) => date >= start && date < end,
        _ => false,
    };
    let hours = if dst { 4 } else { 5 };
    // -4h/-5h are always valid offsets; the UTC fallback can't fire.
    FixedOffset::west_opt(hours * 3600).unwrap_or_else(|| Utc.fix())
}

/// Purge deadline: 11:59:59 PM US Eastern (EST or EDT, whichever is in
/// effect that day) on the third day after archival.
pub fn purge_deadline(archived_at: DateTime<Utc>) -> DateTime<Utc> {
    // The archival's Eastern calendar date (the offset is taken from the
    // UTC date; it can only differ within an hour of a switch night).
    let day = archived_at
        .with_timezone(&eastern(archived_at.date_naive()))
        .date_naive();
    let eod = NaiveTime::from_hms_opt(23, 59, 59).unwrap_or(NaiveTime::MIN);
    let deadline_day = day + Duration::days(3);
    match eastern(deadline_day)
        .from_local_datetime(&deadline_day.and_time(eod))
        .single()
    {
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

    fn utc(year: i32, month: u32, day: u32, hour: u32, min: u32, sec: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(year, month, day, hour, min, sec)
            .unwrap()
    }

    #[test]
    fn deadline_is_est_eod_three_days_out_in_winter() {
        // 2026-01-14 10:00 EST (15:00 UTC) → 2026-01-17 23:59:59 EST.
        let deadline = purge_deadline(utc(2026, 1, 14, 15, 0, 0));
        assert_eq!(deadline, utc(2026, 1, 18, 4, 59, 59));
    }

    #[test]
    fn deadline_is_edt_eod_in_summer() {
        // 2026-09-16 11:00 EDT (15:00 UTC) → 2026-09-19 23:59:59 EDT,
        // an hour earlier in UTC than a fixed UTC-5 gave.
        let deadline = purge_deadline(utc(2026, 9, 16, 15, 0, 0));
        assert_eq!(deadline, utc(2026, 9, 20, 3, 59, 59));
    }

    #[test]
    fn deadline_handles_eastern_date_rollover() {
        // 2026-09-17 03:30 UTC is still 2026-09-16 23:30 EDT, so the
        // purge lands on the 19th.
        let deadline = purge_deadline(utc(2026, 9, 17, 3, 30, 0));
        assert_eq!(deadline, utc(2026, 9, 20, 3, 59, 59));
    }

    #[test]
    fn deadline_follows_the_switch_days() {
        // DST 2026 runs from Sun 8 March to Sun 1 November.
        // Archived in EST, purged on the first EDT day.
        assert_eq!(
            purge_deadline(utc(2026, 3, 5, 15, 0, 0)),
            utc(2026, 3, 9, 3, 59, 59)
        );
        // Archived in EDT, purged on the first EST day.
        assert_eq!(
            purge_deadline(utc(2026, 10, 29, 15, 0, 0)),
            utc(2026, 11, 2, 4, 59, 59)
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
