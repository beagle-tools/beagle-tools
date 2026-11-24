// Tests fail loudly on setup errors; unwrap/expect is the clearest way.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use ::standup::model::{ReportManifest, TicketManifest};

use super::*;

fn date(raw: &str) -> ReportDate {
    ReportDate::parse(raw).unwrap()
}

/// Two reports; the newer one has two tickets in different states.
fn fixture() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path());
    store
        .create_report(date("2026-11-19"), &ReportManifest::default())
        .unwrap();
    let manifest = ReportManifest {
        title: Some("Webhook retries land".into()),
        since: Some("2026-11-19".into()),
        created: None,
    };
    store.create_report(date("2026-11-20"), &manifest).unwrap();
    for (id, state) in [("ABC-2", "Done"), ("ABC-1", "In Review")] {
        let ticket = TicketManifest {
            id: id.into(),
            title: format!("{id} work"),
            url: Some(format!("https://tracker.example.com/{id}")),
            state: Some(state.into()),
            systems: vec!["webhooks".into()],
            prs: vec![],
        };
        store
            .upsert_ticket(date("2026-11-20"), &ticket, false)
            .unwrap();
    }
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
fn the_list_matches_standup_list_json() {
    let (_dir, store) = fixture();
    let body = get(&store, "/v1/standup").unwrap();
    let rows = store.list().unwrap();
    assert_eq!(body["reports"], serde_json::to_value(&rows).unwrap());
    assert_eq!(body["reports"][0]["date"], "2026-11-20");
    assert_eq!(body["reports"][0]["tickets"], 2);
}

#[test]
fn a_report_has_its_overview_and_tickets_most_active_first() {
    let (_dir, store) = fixture();
    let body = get(&store, "/v1/standup/2026-11-20").unwrap();
    assert_eq!(body["weekday"], "Friday");
    assert_eq!(body["title"], "Webhook retries land");
    assert!(body["overview"].is_string());
    let tickets = body["tickets"].as_array().unwrap();
    let ids: Vec<&str> = tickets.iter().map(|t| t["id"].as_str().unwrap()).collect();
    let loaded = store.load(date("2026-11-20")).unwrap();
    let expected: Vec<&str> = loaded.tickets.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(ids, expected);
    let review = tickets.iter().find(|t| t["id"] == "ABC-1").unwrap();
    assert_eq!(review["state_kind"], "review");
    assert_eq!(review["systems"], serde_json::json!(["webhooks"]));
}

#[test]
fn latest_is_the_newest_report() {
    let (_dir, store) = fixture();
    let body = get(&store, "/v1/standup/latest").unwrap();
    assert_eq!(body["date"], "2026-11-20");
}

#[test]
fn bad_and_missing_dates() {
    let (_dir, store) = fixture();
    assert_eq!(
        get(&store, "/v1/standup/2026-02-30").unwrap_err().status,
        400
    );
    assert_eq!(
        get(&store, "/v1/standup/2026-01-01").unwrap_err().status,
        404
    );
    let empty = tempfile::tempdir().unwrap();
    let none = Store::new(empty.path());
    assert_eq!(get(&none, "/v1/standup/latest").unwrap_err().status, 404);
    assert_eq!(
        get(&none, "/v1/standup").unwrap()["reports"],
        serde_json::json!([])
    );
}
