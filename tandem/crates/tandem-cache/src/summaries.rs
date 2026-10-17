//! The one agent-written summary a PR carries. Split from
//! `review_store.rs`, which was over the 400-line rule.

use rusqlite::{params, OptionalExtension};
use tandem_core::github::RepoRef;
use tandem_core::review::PrSummary;
use tandem_core::Result;

use crate::schema::cache_err;
use crate::Cache;

/// The single per-PR summary agents maintain (see [`PrSummary`]).
pub trait SummaryStore {
    /// Upsert the PR's summary — a re-review replaces the previous text
    /// with its revision of it.
    fn put_pr_summary(&self, summary: &PrSummary) -> Result<()>;
    /// The PR's summary, if an agent has written one.
    fn get_pr_summary(&self, repo: &RepoRef, pr_number: u64) -> Result<Option<PrSummary>>;
    /// Remove the PR's summary (no-op if there is none).
    fn delete_pr_summary(&self, repo: &RepoRef, pr_number: u64) -> Result<()>;
}

impl SummaryStore for Cache {
    fn put_pr_summary(&self, summary: &PrSummary) -> Result<()> {
        self.conn
            .execute(
                "INSERT INTO pr_summaries (repo, number, updated_at, json)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(repo, number) DO UPDATE SET
                   updated_at = excluded.updated_at, json = excluded.json",
                params![
                    summary.repo.slug(),
                    summary.pr_number,
                    summary.updated_at.to_rfc3339(),
                    serde_json::to_string(summary)?
                ],
            )
            .map_err(cache_err)?;
        Ok(())
    }

    fn get_pr_summary(&self, repo: &RepoRef, pr_number: u64) -> Result<Option<PrSummary>> {
        let json: Option<String> = self
            .conn
            .query_row(
                "SELECT json FROM pr_summaries WHERE repo = ?1 AND number = ?2",
                params![repo.slug(), pr_number],
                |row| row.get(0),
            )
            .optional()
            .map_err(cache_err)?;
        match json {
            Some(json) => Ok(Some(serde_json::from_str(&json)?)),
            None => Ok(None),
        }
    }

    fn delete_pr_summary(&self, repo: &RepoRef, pr_number: u64) -> Result<()> {
        self.conn
            .execute(
                "DELETE FROM pr_summaries WHERE repo = ?1 AND number = ?2",
                params![repo.slug(), pr_number],
            )
            .map_err(cache_err)?;
        Ok(())
    }
}
