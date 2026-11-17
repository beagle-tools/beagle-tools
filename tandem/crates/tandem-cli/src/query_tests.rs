//! Tests for the queries and the queue, against a real store written by
//! `tandem-cache` (so a schema change that breaks the CLI fails here).

#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use rusqlite::{Connection, OpenFlags};
use serde_json::{json, Value};
use tandem_cache::{Cache, ReviewQueue, ReviewStore};
use tandem_core::github::{PullRequest, RepoRef};
use tandem_core::review::{CommentAuthorKind, CommentSeverity, DiffSide, NewLocalComment};

use crate::{query, queue};

/// A fresh store on disk; removed when dropped.
struct Store(PathBuf);

impl Store {
    fn new() -> (Self, Cache) {
        let dir = std::env::temp_dir().join(format!("tandem-cli-{}", uuid::Uuid::new_v4()));
        let path = dir.join("cache.sqlite3");
        let cache = Cache::open(&path).unwrap();
        (Self(dir), cache)
    }
    fn db(&self) -> PathBuf {
        self.0.join("cache.sqlite3")
    }
    fn read_only(&self) -> Connection {
        Connection::open_with_flags(self.db(), OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap()
    }
}

impl Drop for Store {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn pr(number: u64) -> PullRequest {
    serde_json::from_value(json!({
        "repo": RepoRef::parse("acme/widgets").unwrap(),
        "number": number,
        "title": "t",
        "body": "",
        "author": { "login": "octo", "avatar_url": null },
        "state": "open",
        "draft": false,
        "head_ref": "h",
        "base_ref": "main",
        "head_sha": "abc",
        "additions": 0,
        "deletions": 0,
        "changed_files": 0,
        "created_at": "2026-01-01T00:00:00Z",
        "updated_at": "2026-01-01T00:00:00Z",
        "labels": [],
    }))
    .unwrap()
}

fn comment(parent: Option<String>, body: &str) -> NewLocalComment {
    NewLocalComment {
        repo: RepoRef::parse("acme/widgets").unwrap(),
        pr_number: 7,
        head_sha: "abc".into(),
        path: "src/lib.rs".into(),
        side: DiffSide::New,
        line: 3,
        body: body.into(),
        author_kind: CommentAuthorKind::Agent,
        author_name: "claude".into(),
        severity: CommentSeverity::Suggestion,
        run_id: None,
        tag: None,
        parent_id: parent,
        end_line: None,
        suggestion: None,
        github_comment_id: None,
    }
}

fn finished_review(store: &Store, number: u64) {
    let conn = Connection::open(store.db()).unwrap();
    conn.execute(
        "INSERT INTO agent_runs (run_id, repo, pr_number, started_at, json)
         VALUES ('r1', 'acme/widgets', ?1, '2026-01-01T00:00:00Z', ?2)",
        rusqlite::params![
            number,
            json!({ "purpose": "pr review", "status": "succeeded" }).to_string()
        ],
    )
    .unwrap();
}

fn numbers(value: &Value) -> Vec<u64> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["number"].as_u64().unwrap())
        .collect()
}

#[test]
fn comments_nest_replies_under_their_root() {
    let (store, cache) = Store::new();
    let root = cache.add_comment(comment(None, "root")).unwrap();
    cache.add_comment(comment(Some(root.id), "reply")).unwrap();

    let conn = store.read_only();
    let flat = query::comments(&conn, "acme/widgets", 7, false).unwrap();
    assert_eq!(flat.as_array().unwrap().len(), 2);
    let threads = query::comments(&conn, "acme/widgets", 7, true).unwrap();
    assert_eq!(threads[0]["root"]["body"], "root");
    assert_eq!(threads[0]["replies"][0]["body"], "reply");
}

#[test]
fn summary_is_null_until_written() {
    let (store, _cache) = Store::new();
    let value = query::summary(&store.read_only(), "acme/widgets", 7).unwrap();
    assert_eq!(value, Value::Null);
}

#[test]
fn queue_skips_reviewed_and_queued_prs() {
    let (store, cache) = Store::new();
    cache
        .put_review_requests(&[pr(1), pr(2), pr(3), pr(4)])
        .unwrap();
    finished_review(&store, 1);
    cache
        .enqueue_review("acme/widgets", 2, None, "test")
        .unwrap();
    // A job that failed to start doesn't hide the PR.
    let failed = cache
        .enqueue_review("acme/widgets", 3, None, "test")
        .unwrap();
    cache
        .finish_review_job(&failed, None, Some("no agents configured"))
        .unwrap();

    let conn = store.read_only();
    let mut idle = numbers(&queue::queue(&conn, false).unwrap());
    idle.sort_unstable();
    assert_eq!(idle, vec![3, 4]);
    let mut all = numbers(&queue::queue(&conn, true).unwrap());
    all.sort_unstable();
    assert_eq!(all, vec![2, 3, 4]);
}

#[test]
fn review_records_a_pending_job() {
    let (store, cache) = Store::new();
    let value =
        queue::request_review(&store.db(), "acme/widgets", 9, Some("claude"), "cron").unwrap();
    assert_eq!(value["queued"], true);

    let pending = cache.pending_review_jobs().unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].number, 9);
    assert_eq!(pending[0].agent.as_deref(), Some("claude"));
    assert_eq!(pending[0].requested_by, "cron");
}

#[test]
fn review_refuses_a_bad_slug_and_a_missing_store() {
    let (store, _cache) = Store::new();
    assert!(queue::request_review(&store.db(), "widgets", 1, None, "cron").is_err());
    let missing = store.0.join("nope.sqlite3");
    assert!(queue::request_review(&missing, "acme/widgets", 1, None, "cron").is_err());
    assert!(!missing.exists());
}

#[test]
fn paths_sit_beside_the_database() {
    let value = query::paths(&PathBuf::from("/data/tandem/cache.sqlite3"));
    assert_eq!(value["runs"], "/data/tandem/runs");
    assert_eq!(value["exists"], false);
}

#[test]
fn help_and_version_exit_zero() {
    assert_eq!(
        crate::run(vec!["--version".into()]),
        std::process::ExitCode::SUCCESS
    );
    assert_ne!(
        crate::run(vec!["no-such-command".into()]),
        std::process::ExitCode::SUCCESS
    );
}
