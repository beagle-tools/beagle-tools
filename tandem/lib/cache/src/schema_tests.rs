//! Tests for [`crate::schema`]: an old cache file migrates forward without
//! losing data, and migrating is idempotent.

#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use rusqlite::Connection;

use crate::schema::{migrate, MIGRATIONS};
use crate::store::Cache;

/// A fresh scratch path for one test's database file.
fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tandem-schema-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("cache.db")
}

fn version(conn: &Connection) -> usize {
    let v: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    usize::try_from(v).unwrap()
}

fn table_exists(conn: &Connection, name: &str) -> bool {
    conn.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [name],
        |row| row.get::<_, i64>(0),
    )
    .unwrap()
        == 1
}

/// A database exactly as a build that shipped only the first `n`
/// migrations left it, with one row in a table from the first migration.
fn old_database(path: &PathBuf, n: usize) {
    let conn = Connection::open(path).unwrap();
    for sql in &MIGRATIONS[..n] {
        conn.execute_batch(sql).unwrap();
    }
    conn.pragma_update(None, "user_version", i64::try_from(n).unwrap())
        .unwrap();
    conn.execute(
        "INSERT INTO local_comments (id, repo, pr_number, updated_at, json)
         VALUES ('c1', 'acme/widgets', 7, '2026-01-01T00:00:00Z', '{}')",
        [],
    )
    .unwrap();
}

#[test]
fn an_old_cache_migrates_forward_and_keeps_its_rows() {
    let path = scratch("forward");
    old_database(&path, 3);
    drop(Cache::open(&path).unwrap());

    let conn = Connection::open(&path).unwrap();
    assert_eq!(version(&conn), MIGRATIONS.len());
    for table in [
        "comment_revisions",
        "review_requests",
        "review_jobs",
        "pr_marks",
    ] {
        assert!(table_exists(&conn, table), "{table} was not created");
    }
    let kept: i64 = conn
        .query_row(
            "SELECT count(*) FROM local_comments WHERE id = 'c1'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(kept, 1, "a migration dropped existing data");
}

#[test]
fn every_migration_applies_from_any_shipped_version() {
    for shipped in 1..MIGRATIONS.len() {
        let path = scratch(&format!("from-{shipped}"));
        old_database(&path, shipped);
        let conn = Connection::open(&path).unwrap();
        migrate(&conn).unwrap();
        assert_eq!(version(&conn), MIGRATIONS.len(), "from version {shipped}");
    }
}

#[test]
fn migrating_twice_is_a_no_op() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    // Re-running would fail on CREATE TABLE if any migration ran again.
    migrate(&conn).unwrap();
    assert_eq!(version(&conn), MIGRATIONS.len());
}

#[test]
fn a_newer_cache_is_left_alone() {
    // A database from a newer build (more migrations than this one knows)
    // opens without this build trying to re-apply or undo anything.
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let newer = i64::try_from(MIGRATIONS.len() + 1).unwrap();
    conn.pragma_update(None, "user_version", newer).unwrap();
    migrate(&conn).unwrap();
    assert_eq!(version(&conn), MIGRATIONS.len() + 1);
}

#[test]
fn opening_read_only_never_migrates() {
    let path = scratch("read-only");
    old_database(&path, 2);
    drop(Cache::open_read_only(&path).unwrap());
    let conn = Connection::open(&path).unwrap();
    assert_eq!(version(&conn), 2);
    assert!(!table_exists(&conn, "pr_marks"));
}
