//! PRs you've pointed an agent at. Having reviewed a PR is its own
//! reason to keep seeing it — GitHub only tracks review *requests*, and
//! asking to be added as a reviewer just to follow a PR you already
//! looked at is noise everyone else has to read.

use rusqlite::params;

use crate::schema::cache_err;
use crate::Cache;
use tandem_core::Result;

/// PRs that have at least one agent run.
pub trait ReviewedPrs {
    /// Up to `limit` `(repo slug, PR number)` pairs with agent runs,
    /// ordered by their newest run, most recent first.
    fn list_reviewed_prs(&self, limit: u32) -> Result<Vec<(String, u64)>>;
}

impl ReviewedPrs for Cache {
    fn list_reviewed_prs(&self, limit: u32) -> Result<Vec<(String, u64)>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT repo, pr_number, MAX(started_at) AS latest FROM agent_runs
                 GROUP BY repo, pr_number ORDER BY latest DESC LIMIT ?1",
            )
            .map_err(cache_err)?;
        let rows = stmt
            .query_map(params![limit], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, u64>(1)?))
            })
            .map_err(cache_err)?;
        rows.collect::<std::result::Result<_, _>>()
            .map_err(cache_err)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::ReviewStore;
    use chrono::{Duration, Utc};
    use tandem_core::agent::{AgentRun, RunStatus};

    fn run(id: &str, repo: &str, pr: u64, age_mins: i64) -> AgentRun {
        AgentRun {
            run_id: id.into(),
            agent_name: "claude".into(),
            repo_slug: repo.into(),
            pr_number: pr,
            head_sha: "abc".into(),
            status: RunStatus::Succeeded,
            started_at: Utc::now() - Duration::minutes(age_mins),
            finished_at: None,
            log_path: "/tmp/x".into(),
            comment_count: 0,
            purpose: "pr review".into(),
            target_comment_id: None,
            error: None,
        }
    }

    #[test]
    fn groups_runs_by_pr_newest_first() {
        let cache = Cache::open_in_memory().unwrap();
        cache.put_agent_run(&run("r1", "o/a", 1, 60)).unwrap();
        // Two runs on the same PR collapse to one entry...
        cache.put_agent_run(&run("r2", "o/a", 1, 30)).unwrap();
        cache.put_agent_run(&run("r3", "o/b", 7, 5)).unwrap();

        let got = cache.list_reviewed_prs(10).unwrap();
        assert_eq!(got, vec![("o/b".to_owned(), 7), ("o/a".to_owned(), 1)]);
        // ...and the limit counts PRs, not runs.
        assert_eq!(cache.list_reviewed_prs(1).unwrap().len(), 1);
    }
}
