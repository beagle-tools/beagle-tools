//! Schema migrations. Append new statements to `MIGRATIONS`; never edit
//! an entry that has shipped.

use rusqlite::Connection;
use tandem_core::{Result, TandemError};

pub const MIGRATIONS: &[&str] = &[
    // 001: initial schema
    "
    CREATE TABLE meta (
        key   TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );
    CREATE TABLE pull_requests (
        repo       TEXT NOT NULL,
        number     INTEGER NOT NULL,
        updated_at TEXT NOT NULL,
        json       TEXT NOT NULL,
        PRIMARY KEY (repo, number)
    );
    CREATE TABLE pr_details (
        repo       TEXT NOT NULL,
        number     INTEGER NOT NULL,
        fetched_at TEXT NOT NULL,
        json       TEXT NOT NULL,
        PRIMARY KEY (repo, number)
    );
    CREATE TABLE diffs (
        repo       TEXT NOT NULL,
        number     INTEGER NOT NULL,
        head_sha   TEXT NOT NULL,
        fetched_at TEXT NOT NULL,
        raw        TEXT NOT NULL,
        json       TEXT NOT NULL,
        PRIMARY KEY (repo, number, head_sha)
    );
    CREATE TABLE local_comments (
        id         TEXT PRIMARY KEY,
        repo       TEXT NOT NULL,
        pr_number  INTEGER NOT NULL,
        updated_at TEXT NOT NULL,
        json       TEXT NOT NULL
    );
    CREATE INDEX idx_local_comments_pr ON local_comments (repo, pr_number);
    CREATE TABLE agent_specs (
        name TEXT PRIMARY KEY,
        json TEXT NOT NULL
    );
    CREATE TABLE agent_runs (
        run_id     TEXT PRIMARY KEY,
        repo       TEXT NOT NULL,
        pr_number  INTEGER NOT NULL,
        started_at TEXT NOT NULL,
        json       TEXT NOT NULL
    );
    CREATE INDEX idx_agent_runs_pr ON agent_runs (repo, pr_number);
    ",
    // 002: merged-PR archive (3-day retention before purge)
    "
    CREATE TABLE archived_prs (
        repo        TEXT NOT NULL,
        number      INTEGER NOT NULL,
        archived_at TEXT NOT NULL,
        purge_after TEXT NOT NULL,
        json        TEXT NOT NULL,
        PRIMARY KEY (repo, number)
    );
    ",
    // 003: one agent-written summary per PR
    "
    CREATE TABLE pr_summaries (
        repo       TEXT NOT NULL,
        number     INTEGER NOT NULL,
        updated_at TEXT NOT NULL,
        json       TEXT NOT NULL,
        PRIMARY KEY (repo, number)
    );
    ",
    // 004: edit history for local comments (yours and agents')
    "
    CREATE TABLE comment_revisions (
        id         TEXT PRIMARY KEY,
        comment_id TEXT NOT NULL,
        edited_at  TEXT NOT NULL,
        editor     TEXT NOT NULL,
        body       TEXT NOT NULL
    );
    CREATE INDEX idx_comment_revisions ON comment_revisions (comment_id, edited_at);
    ",
];

/// Run any migrations newer than the connection's `user_version`.
pub fn migrate(conn: &Connection) -> Result<()> {
    let applied: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(cache_err)?;
    let applied = usize::try_from(applied).unwrap_or(0);

    for (idx, sql) in MIGRATIONS.iter().enumerate().skip(applied) {
        conn.execute_batch(sql).map_err(cache_err)?;
        // PRAGMA doesn't support parameter binding; idx comes from
        // enumerate() so the interpolation is safe.
        let version = i64::try_from(idx + 1).map_err(|e| TandemError::Cache(e.to_string()))?;
        conn.pragma_update(None, "user_version", version)
            .map_err(cache_err)?;
        tracing::info!(migration = idx + 1, "applied cache migration");
    }
    Ok(())
}

// By value so it can be passed straight to `map_err(cache_err)`.
#[allow(clippy::needless_pass_by_value)]
pub fn cache_err(e: rusqlite::Error) -> TandemError {
    TandemError::Cache(e.to_string())
}
