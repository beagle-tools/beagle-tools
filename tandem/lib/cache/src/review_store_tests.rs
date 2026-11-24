//! Tests for [`crate::ReviewStore`] — split out to honor the 400-line
//! file limit.

#![allow(clippy::unwrap_used)]

use crate::store::Cache;
use crate::ReviewStore;
use tandem_core::github::RepoRef;
use tandem_core::review::{CommentAuthorKind, CommentSeverity, DiffSide};
use tandem_core::review::{CommentStatus, NewLocalComment};

fn new_comment(repo: &RepoRef) -> NewLocalComment {
    NewLocalComment {
        repo: repo.clone(),
        pr_number: 1,
        head_sha: "abc".into(),
        path: "src/main.rs".into(),
        side: DiffSide::New,
        line: 3,
        body: "consider a match here".into(),
        author_kind: CommentAuthorKind::Agent,
        author_name: "claude".into(),
        severity: CommentSeverity::Suggestion,
        run_id: Some("run-1".into()),
        tag: Some("coding-standards".into()),
        parent_id: None,
        end_line: None,
        suggestion: None,
        github_comment_id: None,
    }
}

#[test]
fn comment_lifecycle() {
    let cache = Cache::open_in_memory().unwrap();
    let repo = RepoRef::parse("o/r").unwrap();

    let c = cache.add_comment(new_comment(&repo)).unwrap();
    assert_eq!(c.status, CommentStatus::Open);

    cache
        .set_comment_status(&c.id, CommentStatus::Accepted)
        .unwrap();
    let listed = cache.list_comments(&repo, 1).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].status, CommentStatus::Accepted);

    cache.delete_comment(&c.id).unwrap();
    assert!(cache.list_comments(&repo, 1).unwrap().is_empty());
}

#[test]
fn deleting_a_root_promotes_the_oldest_reply() {
    let cache = Cache::open_in_memory().unwrap();
    let repo = RepoRef::parse("o/r").unwrap();

    let root = cache.add_comment(new_comment(&repo)).unwrap();
    let mut reply = new_comment(&repo);
    reply.parent_id = Some(root.id.clone());
    let first = cache.add_comment(reply.clone()).unwrap();
    let second = cache.add_comment(reply).unwrap();

    cache.delete_comment(&root.id).unwrap();
    let listed = cache.list_comments(&repo, 1).unwrap();
    assert_eq!(listed.len(), 2);
    let new_root = listed.iter().find(|c| c.id == first.id).unwrap();
    assert_eq!(new_root.parent_id, None);
    let adopted = listed.iter().find(|c| c.id == second.id).unwrap();
    assert_eq!(adopted.parent_id, Some(first.id.clone()));
}

#[test]
fn missing_comment_errors() {
    let cache = Cache::open_in_memory().unwrap();
    assert!(cache
        .set_comment_status("nope", CommentStatus::Resolved)
        .is_err());
}

fn spec_with_env(name: &str) -> tandem_core::agent::AgentSpec {
    serde_json::from_value(serde_json::json!({
        "name": name,
        "runner": {"kind": "claude_headless"},
        "auth": {"kind": "cli_session"},
        "prompt": "review",
        "env": {"API_KEY": "secret-value"},
        "timeout_minutes": 30
    }))
    .unwrap()
}

fn raw_row(cache: &Cache, name: &str) -> String {
    cache
        .conn
        .query_row(
            "SELECT json FROM agent_specs WHERE name = ?1",
            rusqlite::params![name],
            |r| r.get(0),
        )
        .unwrap()
}

#[test]
fn agent_env_never_reaches_the_db_but_reads_back() {
    let cache = Cache::open_in_memory().unwrap();
    cache.put_agent_spec(&spec_with_env("a")).unwrap();
    assert!(!raw_row(&cache, "a").contains("secret-value"));
    let got = cache.list_agent_specs().unwrap();
    assert_eq!(
        got[0].env.get("API_KEY").map(String::as_str),
        Some("secret-value")
    );
    cache.delete_agent_spec("a").unwrap();
    assert!(cache.env_store.get("a").unwrap().is_empty());
}

#[test]
fn a_legacy_row_with_env_is_moved_out_on_first_read() {
    let cache = Cache::open_in_memory().unwrap();
    let legacy = serde_json::to_string(&spec_with_env("old")).unwrap();
    cache
        .conn
        .execute(
            "INSERT INTO agent_specs (name, json) VALUES ('old', ?1)",
            rusqlite::params![legacy],
        )
        .unwrap();
    let got = cache.list_agent_specs().unwrap();
    assert_eq!(
        got[0].env.get("API_KEY").map(String::as_str),
        Some("secret-value")
    );
    assert!(!raw_row(&cache, "old").contains("secret-value"));
}

#[test]
fn the_env_file_is_private() {
    let dir = std::env::temp_dir().join(format!("tandem-env-{}", std::process::id()));
    let cache = Cache::open(&dir.join("cache.sqlite3")).unwrap();
    cache.put_agent_spec(&spec_with_env("a")).unwrap();
    let file = dir.join("agent-env.json");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&file).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
    assert!(std::fs::read_to_string(&file)
        .unwrap()
        .contains("secret-value"));
    let _ = std::fs::remove_dir_all(&dir);
}
