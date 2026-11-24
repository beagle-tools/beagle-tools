// Tests fail loudly on setup errors; unwrap/expect is the clearest way.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use ::recap::comments::{Comment, Threads};
use ::recap::model::{PrState, RecapManifest};

use super::*;

fn manifest(number: u64) -> RecapManifest {
    RecapManifest {
        repo: "acme/widgets".into(),
        number,
        title: format!("ABC-{number}: change {number}"),
        url: format!("https://github.com/acme/widgets/pull/{number}"),
        state: PrState::Open,
        author: Some("octocat".into()),
        branch: Some("feature".into()),
        base: Some("main".into()),
        ticket: Some(format!("ABC-{number}")),
        ticket_url: None,
        additions: 3,
        deletions: 1,
        files: 1,
        created: Some("2026-11-18T15:20:00Z".into()),
        updated: None,
        merged_at: None,
    }
}

const DIFF: &str =
    "diff --git a/a.txt b/a.txt\n--- a/a.txt\n+++ b/a.txt\n@@ -1 +1 @@\n-old\n+new\n";

fn comment(id: &str, author: &str, body: &str) -> Comment {
    Comment {
        id: id.into(),
        author: author.into(),
        bot: false,
        context: false,
        created: "2026-11-19T08:50:00Z".into(),
        body: body.into(),
        url: format!("https://github.com/acme/widgets/pull/42#discussion_r{id}"),
    }
}

/// Two active recaps (42 with synced threads, a read mark and a review)
/// and one archived (7).
fn fixture() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path());
    for n in [42, 43, 7] {
        let id = RecapId::new(&format!("widgets-{n}")).unwrap();
        store.create(&id, &manifest(n), DIFF).unwrap();
    }
    store.archive(&RecapId::new("widgets-7").unwrap()).unwrap();
    let recap_dir = store.recap_dir(&RecapId::new("widgets-42").unwrap());
    let threads = Threads {
        fetched: "2026-11-19T09:05:00Z".into(),
        threads: vec![
            Thread {
                id: "t-901".into(),
                kind: ThreadKind::Inline {
                    path: "src/hooks.rs".into(),
                    line: Some(88),
                    resolved: false,
                    outdated: false,
                },
                comments: vec![comment("901", "reviewer", "Should this back off?")],
            },
            Thread {
                id: "c-5".into(),
                kind: ThreadKind::Conversation,
                comments: vec![comment("5", "reviewer", "Nice.")],
            },
        ],
    };
    let comments_dir = comments::dir(&recap_dir);
    std::fs::create_dir_all(comments_dir.join("reviews")).unwrap();
    std::fs::write(
        comments_dir.join("threads.json"),
        serde_json::to_string(&threads).unwrap(),
    )
    .unwrap();
    std::fs::write(comments_dir.join("read.json"), r#"{"c-5": "5"}"#).unwrap();
    std::fs::write(
        comments_dir.join("reviews/t-901.md"),
        "<!-- covers: 901 -->\n## In short\n\nYes: cap it.\n\n## Detail\n\nUse a backoff.\n",
    )
    .unwrap();
    (dir, store)
}

fn get(store: &Store, path: &str) -> ApiResult {
    let req = Request::get(path, None);
    let rest: Vec<&str> = req
        .path
        .split('/')
        .filter(|s| !s.is_empty())
        .skip(2)
        .collect();
    route(store.root(), &rest, &req)
}

#[test]
fn the_list_matches_recap_list_json() {
    let (_dir, store) = fixture();
    let (rows, _) = store.list_shelf(Shelf::Active).unwrap();
    let body = get(&store, "/v1/recap").unwrap();
    assert_eq!(body["recaps"], serde_json::to_value(&rows).unwrap());
    assert_eq!(body["recaps"].as_array().unwrap().len(), 2);
    assert_eq!(body["recaps"][0]["state"], "open");
    let (archived, _) = store.list_shelf(Shelf::Archived).unwrap();
    let body = get(&store, "/v1/recap?archived=true").unwrap();
    assert_eq!(body["recaps"], serde_json::to_value(&archived).unwrap());
    assert_eq!(body["recaps"][0]["id"], "widgets-7");
    assert_eq!(get(&store, "/v1/recap?archived=x").unwrap_err().status, 400);
}

#[test]
fn one_recap_has_every_manifest_key() {
    let (_dir, store) = fixture();
    let body = get(&store, "/v1/recap/widgets-42").unwrap();
    assert_eq!(body["archived"], false);
    assert_eq!(body["manifest"]["author"], "octocat");
    assert_eq!(body["manifest"]["merged_at"], Value::Null);
    assert!(body["summary"].as_str().unwrap().contains("## TL;DR"));
    let archived = get(&store, "/v1/recap/widgets-7").unwrap();
    assert_eq!(archived["archived"], true);
}

#[test]
fn the_diff_is_returned_whole_when_small() {
    let (_dir, store) = fixture();
    let body = get(&store, "/v1/recap/widgets-43/diff").unwrap();
    assert_eq!(body["diff"], DIFF);
    assert_eq!(body["truncated"], false);
}

#[test]
fn comments_carry_read_state_and_the_agent_review() {
    let (_dir, store) = fixture();
    let body = get(&store, "/v1/recap/widgets-42/comments").unwrap();
    assert_eq!(body["fetched"], "2026-11-19T09:05:00Z");
    let threads = body["threads"].as_array().unwrap();
    assert_eq!(threads.len(), 2);
    let inline = &threads[0];
    assert_eq!(inline["kind"], "inline");
    assert_eq!(inline["place"], "src/hooks.rs:88");
    assert_eq!(inline["read"], false);
    assert_eq!(inline["comments"][0]["author"], "reviewer");
    assert_eq!(inline["review"]["in_short"], "Yes: cap it.");
    assert_eq!(inline["review"]["current"], true);
    assert!(inline["review"]["markdown"]
        .as_str()
        .unwrap()
        .contains("backoff"));
    let conversation = &threads[1];
    assert_eq!(conversation["kind"], "conversation");
    assert_eq!(conversation["path"], Value::Null);
    assert_eq!(conversation["read"], true);
    assert_eq!(conversation["review"], Value::Null);
}

#[test]
fn never_synced_comments_are_empty() {
    let (_dir, store) = fixture();
    let body = get(&store, "/v1/recap/widgets-43/comments").unwrap();
    assert_eq!(body["fetched"], Value::Null);
    assert_eq!(body["threads"], serde_json::json!([]));
}

#[test]
fn bad_and_unknown_ids() {
    let (_dir, store) = fixture();
    assert_eq!(get(&store, "/v1/recap/Bad Id").unwrap_err().status, 400);
    assert_eq!(get(&store, "/v1/recap/widgets-1").unwrap_err().status, 404);
    assert_eq!(
        get(&store, "/v1/recap/widgets-42/x").unwrap_err().status,
        404
    );
}
