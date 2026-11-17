//! The CLI side of the review queue: what's waiting, and asking Tandem
//! to review something. Tandem drains the queue itself (the app's
//! `commands/drain.rs`), so an outside process decides *what* needs a
//! review and Tandem decides *how*.

use std::path::Path;

use rusqlite::Connection;
use serde_json::Value;
use tandem_core::github::RepoRef;

use crate::query::collect;

/// "owner/name" out of a serialized `RepoRef`.
fn slug(pr: &Value) -> String {
    pr.get("repo")
        .map(|r| {
            format!(
                "{}/{}",
                r.get("owner").and_then(Value::as_str).unwrap_or_default(),
                r.get("name").and_then(Value::as_str).unwrap_or_default()
            )
        })
        .unwrap_or_default()
}

fn count(conn: &Connection, sql: &str, repo: &str, number: u64) -> Result<u32, String> {
    conn.query_row(sql, rusqlite::params![repo, number], |row| row.get(0))
        .map_err(|e| e.to_string())
}

/// What's waiting on you that nothing has reviewed yet. "Reviewed"
/// means a finished `pr review` run exists for it — the same thing the
/// review-status list means by its green badge. Unless `include_queued`,
/// PRs with a pending or started (not failed) job are left out too, so
/// running a cron twice doesn't review the same PR twice.
pub(crate) fn queue(conn: &Connection, include_queued: bool) -> Result<Value, String> {
    let requests = collect(
        conn,
        "SELECT json FROM review_requests ORDER BY fetched_at DESC, repo, number",
        &[],
    )
    .map_err(|e| missing_table_hint(&e))?;
    let mut out = Vec::new();
    for pr in requests.as_array().cloned().unwrap_or_default() {
        let repo = slug(&pr);
        let number = pr.get("number").and_then(Value::as_u64).unwrap_or_default();
        let reviewed = count(
            conn,
            "SELECT COUNT(*) FROM agent_runs
             WHERE repo = ?1 AND pr_number = ?2
               AND json_extract(json, '$.purpose') = 'pr review'
               AND json_extract(json, '$.status') NOT IN ('starting', 'running')",
            &repo,
            number,
        )?;
        if reviewed > 0 {
            continue;
        }
        // A request already queued isn't idle — asking twice would
        // start two reviews of the same PR. A job that failed to start
        // doesn't count, so the PR comes back.
        if !include_queued {
            let queued = count(
                conn,
                "SELECT COUNT(*) FROM review_jobs WHERE repo = ?1 AND number = ?2
                   AND (started_at IS NULL OR error IS NULL)",
                &repo,
                number,
            )?;
            if queued > 0 {
                continue;
            }
        }
        out.push(pr);
    }
    Ok(Value::Array(out))
}

/// Record a review request. Opened read-write on purpose and only for
/// this: the queue is an inbox for outside processes, not review data.
pub(crate) fn request_review(
    db: &Path,
    repo: &str,
    number: u64,
    agent: Option<&str>,
    by: &str,
) -> Result<Value, String> {
    // Checked here so a malformed slug fails at the door rather than as
    // a failed job inside the app.
    if RepoRef::parse(repo).is_none() {
        return Err(format!("not a repo slug: {repo} (want owner/name)"));
    }
    // No CREATE flag: a missing database is an error, never a new file.
    let conn = Connection::open_with_flags(db, rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE)
        .map_err(|e| format!("can't open {}: {e}", db.display()))?;
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO review_jobs (id, repo, number, agent, requested_by, requested_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![id, repo, number, agent, by, chrono::Utc::now().to_rfc3339()],
    )
    .map_err(|e| missing_table_hint(&e.to_string()))?;
    Ok(serde_json::json!({
        "queued": true,
        "id": id,
        "repo": repo,
        "pr": number,
        "agent": agent,
        "note": "Tandem starts this within ~20s while it is running",
    }))
}

/// The queue tables arrive with a schema migration, so a store an older
/// Tandem wrote doesn't have them yet; say what to do about it.
fn missing_table_hint(error: &str) -> String {
    if error.contains("no such table") {
        format!("{error} — this store predates the review queue; open an up-to-date Tandem once")
    } else {
        error.to_owned()
    }
}
