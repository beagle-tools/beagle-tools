// Tests fail loudly on setup errors; unwrap/expect/panic is the clearest way.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use beagle_scout::model::{Slug, TaskStatus};
use serde_json::Value;

use super::*;

fn slug(s: &str) -> Slug {
    Slug::parse(s).unwrap()
}

/// A store with one active project: three tasks, the first done with next
/// steps written, the second being worked.
fn fixture() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path().join("scout"));
    store
        .create_project(
            &slug("rate-limit"),
            "Rate-limit the API",
            vec!["ABC-1".into()],
        )
        .unwrap();
    for (s, title) in [
        ("map", "Map it"),
        ("allowlist", "Add the allowlist"),
        ("ship", "Ship"),
    ] {
        store
            .add_task("rate-limit", &slug(s), title, vec![])
            .unwrap();
    }
    let done = store
        .set_task_status("rate-limit", "1", TaskStatus::Done)
        .unwrap()
        .0;
    let text = std::fs::read_to_string(&done.path).unwrap().replace(
        "<Filled in when the task closes or work pauses: what should happen next, and\nanything the next task (or the next session) needs to know to pick it up.>",
        "Next is the allowlist.",
    );
    std::fs::write(&done.path, text).unwrap();
    store
        .set_task_status("rate-limit", "allowlist", TaskStatus::Working)
        .unwrap();
    store
        .append_log("rate-limit", Some("2"), "starting the allowlist")
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
fn projects_match_scout_list_json() {
    let (_dir, store) = fixture();
    let body = get(&store, "/v1/scout/projects").unwrap();
    let listing = store.list().unwrap();
    let projects = body["projects"].as_array().unwrap();
    assert_eq!(projects.len(), listing.projects.len());
    for (served, p) in projects.iter().zip(&listing.projects) {
        let Value::Object(cli_row) = p.list_json() else {
            panic!("list row is an object")
        };
        for (key, value) in cli_row {
            if key != "dir" {
                assert_eq!(&served[&key], &value, "{key}");
            }
        }
        assert!(served.get("dir").is_none(), "no local paths");
    }
    let p = &projects[0];
    assert_eq!(p["status"], "active");
    assert_eq!(p["tasks_closed"], 1);
    assert_eq!(p["tasks_total"], 3);
    assert_eq!(p["working"], serde_json::json!(["02-allowlist"]));
    assert!(p["created"].as_str().unwrap().ends_with('Z'));
    assert!(p["last_activity"].is_string());
}

#[test]
fn a_project_has_its_tasks_markdown_and_handoff() {
    let (_dir, store) = fixture();
    let p = get(&store, "/v1/scout/projects/rate-limit").unwrap();
    assert_eq!(p["title"], "Rate-limit the API");
    assert!(p["markdown"].as_str().unwrap().starts_with("## Goal"));
    let tasks = p["tasks"].as_array().unwrap();
    let ids: Vec<&str> = tasks.iter().map(|t| t["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["01-map", "02-allowlist", "03-ship"]);
    assert_eq!(tasks[0]["status"], "done");
    assert_eq!(tasks[1]["status"], "working");
    assert_eq!(tasks[1]["checklist"]["total"], 2);
    assert_eq!(p["handoff"]["task"], "01-map");
    assert_eq!(p["handoff"]["markdown"], "Next is the allowlist.");
}

#[test]
fn a_task_resolves_by_number_id_or_slug() {
    let (_dir, store) = fixture();
    for reference in ["2", "02", "02-allowlist", "allowlist"] {
        let t = get(
            &store,
            &format!("/v1/scout/projects/rate-limit/tasks/{reference}"),
        )
        .unwrap();
        assert_eq!(t["id"], "02-allowlist", "{reference}");
        assert_eq!(t["project"], "rate-limit");
        assert!(t["markdown"].as_str().unwrap().contains("## Steps"));
        assert_eq!(t["next_steps"], Value::Null);
    }
    let done = get(&store, "/v1/scout/projects/rate-limit/tasks/1").unwrap();
    assert_eq!(done["next_steps"], "Next is the allowlist.");
}

#[test]
fn the_log_tail_is_capped() {
    let (_dir, store) = fixture();
    let all = get(&store, "/v1/scout/projects/rate-limit/log").unwrap();
    let lines = all["lines"].as_array().unwrap();
    assert!(lines.len() > 2);
    assert_eq!(all["truncated"], false);
    assert!(lines
        .last()
        .unwrap()
        .as_str()
        .unwrap()
        .contains("starting the allowlist"));
    let one = get(&store, "/v1/scout/projects/rate-limit/log?lines=1").unwrap();
    assert_eq!(one["lines"].as_array().unwrap().len(), 1);
    assert_eq!(one["truncated"], true);
    let err = get(&store, "/v1/scout/projects/rate-limit/log?lines=x").unwrap_err();
    assert_eq!(err.status, 400);
}

#[test]
fn bad_and_unknown_ids_are_400_and_404() {
    let (_dir, store) = fixture();
    assert_eq!(
        get(&store, "/v1/scout/projects/Nope").unwrap_err().status,
        400
    );
    assert_eq!(
        get(&store, "/v1/scout/projects/nope").unwrap_err().status,
        404
    );
    assert_eq!(
        get(&store, "/v1/scout/projects/rate-limit/tasks/9")
            .unwrap_err()
            .status,
        404
    );
    assert_eq!(get(&store, "/v1/scout/other").unwrap_err().status, 404);
}

#[test]
fn reading_never_writes() {
    let (_dir, store) = fixture();
    let log = store.project("rate-limit").unwrap().log_file();
    let before = std::fs::read_to_string(&log).unwrap();
    for path in [
        "/v1/scout/projects",
        "/v1/scout/projects/rate-limit",
        "/v1/scout/projects/rate-limit/tasks/2",
        "/v1/scout/projects/rate-limit/log",
    ] {
        get(&store, path).unwrap();
    }
    assert_eq!(std::fs::read_to_string(&log).unwrap(), before);
}
