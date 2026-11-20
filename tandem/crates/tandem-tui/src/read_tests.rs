// Tests panic on a broken fixture; that is the failure mode we want.
#![allow(clippy::unwrap_used)]

use chrono::Utc;
use tandem_cache::ReviewQueue;
use tandem_core::review::{NewLocalComment, PrSummary};

use super::*;

fn new_comment(body: &str, parent: Option<&str>) -> NewLocalComment {
    serde_json::from_value(json!({
        "repo": {"owner": "o", "name": "r"}, "pr_number": 1, "head_sha": "s",
        "path": "src/a.rs", "side": "new", "line": 3, "end_line": null,
        "body": body, "suggestion": null, "author_kind": "agent",
        "author_name": "claude", "severity": "issue", "run_id": null,
        "tag": null, "parent_id": parent, "github_comment_id": null
    }))
    .unwrap()
}

/// Two queued PRs; #1 has a summary with a verdict and a comment thread.
fn fixture(cache: &Cache) {
    cache.enqueue_review("o/r", 1, None, "tui").unwrap();
    cache.enqueue_review("o/r", 2, None, "tui").unwrap();
    let root = cache
        .add_comment(new_comment("Cap the retries.", None))
        .unwrap();
    cache
        .add_comment(new_comment("Agreed.", Some(&root.id)))
        .unwrap();
    cache
        .put_pr_summary(&PrSummary {
            repo: RepoRef::parse("o/r").unwrap(),
            pr_number: 1,
            body: "**Mergeable with suggestions** — small things.\n\nDetail.".into(),
            agent_name: "claude".into(),
            run_id: "r1".into(),
            head_sha: "s".into(),
            updated_at: Utc::now(),
            thread_root_id: None,
        })
        .unwrap();
}

#[test]
fn the_queue_is_the_list_rows_in_order() {
    let cache = Cache::open_in_memory().unwrap();
    fixture(&cache);
    let body = queue(&cache).unwrap();
    let served: Vec<(String, u64, String)> = body["prs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            (
                p["repo"].as_str().unwrap().to_owned(),
                p["number"].as_u64().unwrap(),
                p["stage"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    let listed: Vec<(String, u64, String)> = state::rows(&cache)
        .unwrap()
        .into_iter()
        .map(|r| (r.pr.repo.slug(), r.pr.number, r.stage.label().to_owned()))
        .collect();
    assert_eq!(served, listed);
    let one = body["prs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["number"] == 1)
        .unwrap();
    assert_eq!(one["verdict"], "mergeable-with-suggestions");
    assert_eq!(one["open_comments"], 1);
    assert_eq!(one["approved"], false);
}

#[test]
fn a_pr_has_its_summary_and_nested_comments() {
    let cache = Cache::open_in_memory().unwrap();
    fixture(&cache);
    let body = pr(&cache, "o/r", 1).unwrap();
    assert_eq!(body["verdict"], "mergeable-with-suggestions");
    assert_eq!(body["summary"]["agent"], "claude");
    assert_eq!(body["title"], Value::Null);
    let comments = body["comments"].as_array().unwrap();
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0]["body"], "Cap the retries.");
    assert_eq!(comments[0]["severity"], "issue");
    assert_eq!(comments[0]["status"], "open");
    assert_eq!(comments[0]["posted"], false);
    assert_eq!(comments[0]["replies"][0]["body"], "Agreed.");
    let empty = pr(&cache, "o/r", 2).unwrap();
    assert_eq!(empty["summary"], Value::Null);
    assert_eq!(empty["comments"], json!([]));
    assert!(pr(&cache, "not-a-slug", 1).is_err());
}

#[test]
fn a_missing_store_is_none_and_never_created() {
    let dir = std::env::temp_dir().join(format!("tandem-read-{}", uuid::Uuid::new_v4()));
    let path = dir.join("cache.sqlite3");
    assert!(open(&path).unwrap().is_none());
    assert!(!path.exists());
}

#[test]
fn an_existing_store_opens_read_only() {
    let dir = std::env::temp_dir().join(format!("tandem-read-{}", uuid::Uuid::new_v4()));
    let path = dir.join("cache.sqlite3");
    fixture(&Cache::open(&path).unwrap());
    let cache = open(&path).unwrap().unwrap();
    assert_eq!(queue(&cache).unwrap()["prs"].as_array().unwrap().len(), 2);
    assert!(cache.enqueue_review("o/r", 3, None, "tui").is_err());
    std::fs::remove_dir_all(&dir).unwrap();
}
