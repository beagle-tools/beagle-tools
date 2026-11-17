//! The read queries. Rows are JSON blobs with indexed key columns, so
//! most of this is "select the blob, hand it back" — the value is
//! knowing which table and which key.

use std::path::Path;

use rusqlite::{Connection, Row};
use serde_json::Value;

/// Print `value` as one line of JSON, or indented with `pretty`.
pub(crate) fn print(value: &Value, pretty: bool) {
    let text = if pretty {
        serde_json::to_string_pretty(value)
    } else {
        serde_json::to_string(value)
    };
    match text {
        Ok(text) => println!("{text}"),
        Err(e) => eprintln!("tandem-cli: couldn't render JSON: {e}"),
    }
}

fn blob(row: &Row<'_>) -> rusqlite::Result<Value> {
    let raw: String = row.get(0)?;
    // A blob that isn't JSON degrades to null rather than failing the
    // whole listing.
    Ok(serde_json::from_str(&raw).unwrap_or(Value::Null))
}

/// Every row's first column, parsed as JSON.
pub(crate) fn collect(
    conn: &Connection,
    sql: &str,
    params: &[&dyn rusqlite::ToSql],
) -> Result<Value, String> {
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(params, blob).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(Value::Array(out))
}

/// Where everything lives, derived from the database path.
pub(crate) fn paths(db: &Path) -> Value {
    let data = db.parent().map(Path::to_path_buf).unwrap_or_default();
    serde_json::json!({
        "database": db,
        "exists": db.exists(),
        "runs": data.join("runs"),
        "worktrees": data.join("worktrees"),
        // Settings live under the config dir, which is the same as the
        // data dir everywhere except Linux.
        "settings": dirs::config_dir()
            .map(|c| c.join(data.file_name().unwrap_or_default()).join("settings.json")),
    })
}

/// Cached PRs, most recently updated first.
pub(crate) fn prs(conn: &Connection, repo: Option<&str>, limit: u32) -> Result<Value, String> {
    match repo {
        Some(repo) => collect(
            conn,
            "SELECT json FROM pull_requests WHERE repo = ?1
             ORDER BY json_extract(json, '$.updated_at') DESC LIMIT ?2",
            &[&repo, &limit],
        ),
        None => collect(
            conn,
            "SELECT json FROM pull_requests
             ORDER BY json_extract(json, '$.updated_at') DESC LIMIT ?1",
            &[&limit],
        ),
    }
}

/// Local comments on one PR, oldest first; with `threads`, roots in
/// order with their replies nested as `{root, replies}`.
pub(crate) fn comments(
    conn: &Connection,
    repo: &str,
    pr: u64,
    threads: bool,
) -> Result<Value, String> {
    let flat = collect(
        conn,
        "SELECT json FROM local_comments WHERE repo = ?1 AND pr_number = ?2
         ORDER BY json_extract(json, '$.created_at') ASC",
        &[&repo, &pr],
    )?;
    if !threads {
        return Ok(flat);
    }
    // Roots in order, each carrying its replies — the shape a reader
    // actually wants, rather than one they have to reassemble.
    let items = flat.as_array().cloned().unwrap_or_default();
    let roots: Vec<Value> = items
        .iter()
        .filter(|c| c.get("parent_id").is_none_or(Value::is_null))
        .map(|root| {
            let id = root.get("id").and_then(Value::as_str).unwrap_or_default();
            let replies: Vec<Value> = items
                .iter()
                .filter(|c| c.get("parent_id").and_then(Value::as_str) == Some(id))
                .cloned()
                .collect();
            serde_json::json!({ "root": root, "replies": replies })
        })
        .collect();
    Ok(Value::Array(roots))
}

/// The PR's agent summary, or `null`.
pub(crate) fn summary(conn: &Connection, repo: &str, pr: u64) -> Result<Value, String> {
    let rows = collect(
        conn,
        "SELECT json FROM pr_summaries WHERE repo = ?1 AND number = ?2",
        &[&repo, &pr],
    )?;
    Ok(rows
        .as_array()
        .and_then(|r| r.first())
        .cloned()
        .unwrap_or(Value::Null))
}

/// Agent runs, newest first, optionally narrowed to a repo or one PR.
pub(crate) fn runs(
    conn: &Connection,
    repo: Option<&str>,
    pr: Option<u64>,
    limit: u32,
) -> Result<Value, String> {
    match (repo, pr) {
        (Some(repo), Some(pr)) => collect(
            conn,
            "SELECT json FROM agent_runs WHERE repo = ?1 AND pr_number = ?2
             ORDER BY started_at DESC LIMIT ?3",
            &[&repo, &pr, &limit],
        ),
        (Some(repo), None) => collect(
            conn,
            "SELECT json FROM agent_runs WHERE repo = ?1 ORDER BY started_at DESC LIMIT ?2",
            &[&repo, &limit],
        ),
        _ => collect(
            conn,
            "SELECT json FROM agent_runs ORDER BY started_at DESC LIMIT ?1",
            &[&limit],
        ),
    }
}
