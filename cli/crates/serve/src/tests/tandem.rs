// Tests fail loudly on setup errors; unwrap/expect is the clearest way.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use serde_json::Value;
use tandem_cache::{Cache, ReviewQueue};

use super::*;

fn get(db: &Path, path: &str) -> ApiResult {
    let req = Request::get(path, None);
    let rest: Vec<&str> = req
        .path
        .split('/')
        .filter(|s| !s.is_empty())
        .skip(2)
        .collect();
    route(db, &rest, &req)
}

#[test]
fn a_missing_store_is_an_empty_queue_and_is_never_created() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("tandem-tui/cache.sqlite3");
    assert_eq!(get(&db, "/v1/tandem/queue").unwrap(), json!({ "prs": [] }));
    assert_eq!(
        get(&db, "/v1/tandem/prs/acme/widgets/42")
            .unwrap_err()
            .status,
        404
    );
    assert!(!db.exists());
}

#[test]
fn the_queue_matches_the_review_tuis_rows() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("cache.sqlite3");
    {
        let cache = Cache::open(&db).unwrap();
        cache
            .enqueue_review("acme/widgets", 42, None, "cli")
            .unwrap();
        cache
            .enqueue_review("acme/widgets", 43, None, "cli")
            .unwrap();
    }
    let body = get(&db, "/v1/tandem/queue").unwrap();
    let cache = read::open(&db).unwrap().unwrap();
    assert_eq!(body, read::queue(&cache).unwrap());
    let prs = body["prs"].as_array().unwrap();
    assert_eq!(prs.len(), 2);
    assert!(prs.iter().all(|p| p["stage"] == "queued"));
    let one = get(&db, "/v1/tandem/prs/acme/widgets/42").unwrap();
    assert_eq!(one["repo"], "acme/widgets");
    assert_eq!(one["summary"], Value::Null);
}

#[test]
fn bad_repos_and_numbers_are_400() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("cache.sqlite3");
    for path in [
        "/v1/tandem/prs/acme/widgets/0",
        "/v1/tandem/prs/acme/widgets/x",
        "/v1/tandem/prs/ac%20me/widgets/1",
    ] {
        assert_eq!(get(&db, path).unwrap_err().status, 400, "{path}");
    }
    assert_eq!(get(&db, "/v1/tandem/other").unwrap_err().status, 404);
}
