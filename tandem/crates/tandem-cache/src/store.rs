//! GitHub-data side of the cache: PR lists, detail bundles, diffs, and
//! sync bookkeeping. Local review data lives in [`crate::review_store`].

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use tandem_core::diff::FileDiff;
use tandem_core::github::{PrDetail, PullRequest, RepoRef};
use tandem_core::{Result, TandemError};

use crate::schema::{cache_err, migrate};

/// Handle to the cache database. Not thread-safe by itself — the app
/// layer wraps it in a mutex.
/// Handle to Tandem's SQLite cache file. Every store trait in this crate
/// is implemented on it, so all data shares one connection and database.
#[derive(Debug)]
pub struct Cache {
    pub(crate) conn: Connection,
}

impl Cache {
    /// Open (creating the file and its parent dirs if needed) in WAL mode
    /// and apply any pending schema migrations.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path).map_err(cache_err)?;
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(cache_err)?;
        migrate(&conn)?;
        Ok(Self { conn })
    }

    /// In-memory cache for tests.
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().map_err(cache_err)?;
        migrate(&conn)?;
        Ok(Self { conn })
    }

    // ----- pull request lists -------------------------------------------

    /// Replace the cached open-PR list for a repo.
    pub fn put_pull_requests(&mut self, repo: &RepoRef, prs: &[PullRequest]) -> Result<()> {
        let tx = self.conn.transaction().map_err(cache_err)?;
        tx.execute(
            "DELETE FROM pull_requests WHERE repo = ?1",
            params![repo.slug()],
        )
        .map_err(cache_err)?;
        for pr in prs {
            tx.execute(
                "INSERT INTO pull_requests (repo, number, updated_at, json)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    repo.slug(),
                    pr.number,
                    pr.updated_at.to_rfc3339(),
                    serde_json::to_string(pr)?
                ],
            )
            .map_err(cache_err)?;
        }
        tx.commit().map_err(cache_err)?;
        self.touch_sync(&format!("prs:{}", repo.slug()))
    }

    /// Merge one further page of PRs into the cache (no delete).
    pub fn append_pull_requests(&mut self, repo: &RepoRef, prs: &[PullRequest]) -> Result<()> {
        let tx = self.conn.transaction().map_err(cache_err)?;
        for pr in prs {
            tx.execute(
                "INSERT OR REPLACE INTO pull_requests (repo, number, updated_at, json)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    repo.slug(),
                    pr.number,
                    pr.updated_at.to_rfc3339(),
                    serde_json::to_string(pr)?
                ],
            )
            .map_err(cache_err)?;
        }
        tx.commit().map_err(cache_err)?;
        Ok(())
    }

    /// Cached PR list for a repo, most recently updated first.
    pub fn get_pull_requests(&self, repo: &RepoRef) -> Result<Vec<PullRequest>> {
        let mut stmt = self
            .conn
            .prepare("SELECT json FROM pull_requests WHERE repo = ?1 ORDER BY updated_at DESC")
            .map_err(cache_err)?;
        let rows = stmt
            .query_map(params![repo.slug()], |row| row.get::<_, String>(0))
            .map_err(cache_err)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(serde_json::from_str(&row.map_err(cache_err)?)?);
        }
        Ok(out)
    }

    // ----- PR detail bundles --------------------------------------------

    /// Insert or replace the detail bundle for one PR.
    pub fn put_pr_detail(&self, repo: &RepoRef, detail: &PrDetail) -> Result<()> {
        self.conn
            .execute(
                "INSERT OR REPLACE INTO pr_details (repo, number, fetched_at, json)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    repo.slug(),
                    detail.pull_request.number,
                    Utc::now().to_rfc3339(),
                    serde_json::to_string(detail)?
                ],
            )
            .map_err(cache_err)?;
        Ok(())
    }

    /// Cached detail bundle for one PR, if it was ever synced.
    pub fn get_pr_detail(&self, repo: &RepoRef, number: u64) -> Result<Option<PrDetail>> {
        self.get_blob(
            "SELECT json FROM pr_details WHERE repo = ?1 AND number = ?2",
            params![repo.slug(), number],
        )
    }

    // ----- diffs ----------------------------------------------------------

    /// Store a PR's diff for one head SHA, both raw (for agent prompts) and
    /// parsed (for the UI). Older SHAs' rows are kept.
    pub fn put_diff(
        &self,
        repo: &RepoRef,
        number: u64,
        head_sha: &str,
        raw: &str,
        files: &[FileDiff],
    ) -> Result<()> {
        self.conn
            .execute(
                "INSERT OR REPLACE INTO diffs (repo, number, head_sha, fetched_at, raw, json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    repo.slug(),
                    number,
                    head_sha,
                    Utc::now().to_rfc3339(),
                    raw,
                    serde_json::to_string(files)?
                ],
            )
            .map_err(cache_err)?;
        Ok(())
    }

    /// The raw unified diff text (used to build agent prompts).
    pub fn get_raw_diff(
        &self,
        repo: &RepoRef,
        number: u64,
        head_sha: &str,
    ) -> Result<Option<String>> {
        self.conn
            .query_row(
                "SELECT raw FROM diffs WHERE repo = ?1 AND number = ?2 AND head_sha = ?3",
                params![repo.slug(), number, head_sha],
                |row| row.get(0),
            )
            .optional()
            .map_err(cache_err)
    }

    /// The parsed diff for an exact head SHA; `None` when that SHA hasn't
    /// been fetched yet.
    pub fn get_diff(
        &self,
        repo: &RepoRef,
        number: u64,
        head_sha: &str,
    ) -> Result<Option<Vec<FileDiff>>> {
        self.get_blob(
            "SELECT json FROM diffs WHERE repo = ?1 AND number = ?2 AND head_sha = ?3",
            params![repo.slug(), number, head_sha],
        )
    }

    // ----- sync bookkeeping ----------------------------------------------

    /// Stamp `key` (e.g. `prs:owner/name`) as synced now.
    pub fn touch_sync(&self, key: &str) -> Result<()> {
        self.conn
            .execute(
                "INSERT OR REPLACE INTO meta (key, value) VALUES (?1, ?2)",
                params![format!("synced:{key}"), Utc::now().to_rfc3339()],
            )
            .map_err(cache_err)?;
        Ok(())
    }

    /// When `key` was last stamped by [`Cache::touch_sync`]; `None` if never.
    pub fn last_synced(&self, key: &str) -> Result<Option<DateTime<Utc>>> {
        let value: Option<String> = self
            .conn
            .query_row(
                "SELECT value FROM meta WHERE key = ?1",
                params![format!("synced:{key}")],
                |row| row.get(0),
            )
            .optional()
            .map_err(cache_err)?;
        match value {
            None => Ok(None),
            Some(v) => DateTime::parse_from_rfc3339(&v)
                .map(|dt| Some(dt.with_timezone(&Utc)))
                .map_err(|e| TandemError::Cache(format!("bad timestamp in meta: {e}"))),
        }
    }

    fn get_blob<T: serde::de::DeserializeOwned, P: rusqlite::Params>(
        &self,
        sql: &str,
        params: P,
    ) -> Result<Option<T>> {
        let json: Option<String> = self
            .conn
            .query_row(sql, params, |row| row.get(0))
            .optional()
            .map_err(cache_err)?;
        match json {
            None => Ok(None),
            Some(j) => Ok(Some(serde_json::from_str(&j)?)),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use tandem_core::github::{PrState, User};

    fn sample_pr(repo: &RepoRef, number: u64) -> PullRequest {
        PullRequest {
            repo: repo.clone(),
            number,
            title: format!("PR {number}"),
            body: String::new(),
            state: PrState::Open,
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
        }
    }

    #[test]
    fn roundtrips_pull_requests() {
        let mut cache = Cache::open_in_memory().unwrap();
        let repo = RepoRef::parse("o/r").unwrap();
        cache
            .put_pull_requests(&repo, &[sample_pr(&repo, 1), sample_pr(&repo, 2)])
            .unwrap();

        let got = cache.get_pull_requests(&repo).unwrap();
        assert_eq!(got.len(), 2);
        assert!(cache
            .last_synced(&format!("prs:{}", repo.slug()))
            .unwrap()
            .is_some());

        // replace semantics: a new put wins wholesale
        cache
            .put_pull_requests(&repo, &[sample_pr(&repo, 3)])
            .unwrap();
        let got = cache.get_pull_requests(&repo).unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].number, 3);
    }

    #[test]
    fn diff_cache_keyed_by_sha() {
        let cache = Cache::open_in_memory().unwrap();
        let repo = RepoRef::parse("o/r").unwrap();
        cache.put_diff(&repo, 1, "sha1", "+ raw", &[]).unwrap();
        assert!(cache.get_diff(&repo, 1, "sha1").unwrap().is_some());
        assert!(cache.get_diff(&repo, 1, "sha2").unwrap().is_none());
        assert_eq!(
            cache.get_raw_diff(&repo, 1, "sha1").unwrap().unwrap(),
            "+ raw"
        );
    }
}
