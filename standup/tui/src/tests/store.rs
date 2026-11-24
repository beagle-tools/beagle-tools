//! Tests for `store`.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::*;

fn date(s: &str) -> ReportDate {
    ReportDate::parse(s).unwrap()
}

fn ticket(id: &str, state: &str) -> TicketManifest {
    TicketManifest {
        id: id.to_owned(),
        title: format!("title of {id}"),
        url: Some(format!("https://tracker.example.com/issue/{id}")),
        state: Some(state.to_owned()),
        systems: vec!["api-gateway".to_owned()],
        prs: Vec::new(),
    }
}

#[test]
fn missing_root_is_an_empty_store() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::new(tmp.path().join("nope"));
    assert_eq!(store.dates().unwrap(), Vec::<ReportDate>::new());
    assert!(store.latest().unwrap().is_none());
}

#[test]
fn create_list_and_load_round_trip() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::new(tmp.path());
    let manifest = ReportManifest {
        title: Some("gateway week".to_owned()),
        since: Some("2026-10-24".to_owned()),
        created: None,
    };
    store
        .create_report(date("2026-10-24"), &ReportManifest::default())
        .unwrap();
    store.create_report(date("2026-10-27"), &manifest).unwrap();
    fs::create_dir(tmp.path().join("standups/not-a-date")).unwrap();

    store
        .upsert_ticket(date("2026-10-27"), &ticket("abc-2", "Done"), false)
        .unwrap();
    store
        .upsert_ticket(date("2026-10-27"), &ticket("ABC-9", "In Progress"), false)
        .unwrap();

    let list = store.list().unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].date, "2026-10-27");
    assert_eq!(list[0].tickets, 2);
    assert_eq!(
        store.previous(date("2026-10-27")).unwrap(),
        Some(date("2026-10-24"))
    );

    let report = store.load(date("2026-10-27")).unwrap();
    assert_eq!(report.manifest, manifest);
    assert!(report.overview.contains("Since last standup"));
    // In-progress work sorts ahead of done work; ids are upper-cased.
    let ids: Vec<&str> = report.tickets.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(ids, ["ABC-9", "ABC-2"]);
    assert!(report.tickets[0]
        .summary
        .contains("Where this sits in the stack"));
    assert_eq!(report.warnings, Vec::<String>::new());
}

#[test]
fn duplicates_are_refused_unless_updating() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::new(tmp.path());
    let d = date("2026-10-27");
    store.create_report(d, &ReportManifest::default()).unwrap();
    assert!(matches!(
        store.create_report(d, &ReportManifest::default()),
        Err(Error::AlreadyExists(_))
    ));
    let dir = store
        .upsert_ticket(d, &ticket("ABC-1", "Todo"), false)
        .unwrap();
    fs::write(dir.join("summary.md"), "hand written").unwrap();
    assert!(store
        .upsert_ticket(d, &ticket("ABC-1", "Done"), false)
        .is_err());
    store
        .upsert_ticket(d, &ticket("ABC-1", "Done"), true)
        .unwrap();
    let report = store.load(d).unwrap();
    assert_eq!(report.tickets[0].manifest.state.as_deref(), Some("Done"));
    // Updating the manifest never clobbers the write-up.
    assert_eq!(report.tickets[0].summary, "hand written");
}

#[test]
fn ticket_needs_an_existing_report() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::new(tmp.path());
    assert!(matches!(
        store.upsert_ticket(date("2026-10-27"), &ticket("ABC-1", "Todo"), false),
        Err(Error::NoSuchReport(_))
    ));
}

#[test]
fn malformed_files_degrade_to_warnings() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::new(tmp.path());
    let d = date("2026-10-27");
    let dir = store.create_report(d, &ReportManifest::default()).unwrap();
    fs::write(dir.join("report.toml"), "this is = = not toml").unwrap();
    let bad = dir.join("tickets/ABC-1");
    fs::create_dir_all(&bad).unwrap();
    fs::write(bad.join("ticket.toml"), "nope = true").unwrap();
    fs::create_dir_all(dir.join("tickets/ABC-2")).unwrap(); // no manifest yet
    fs::write(dir.join("diagrams/01-a.txt"), "\u{1b}[31mBUG\u{1b}[0m").unwrap();
    fs::write(dir.join("diagrams/notes.md"), "ignored").unwrap();
    fs::write(dir.join("overview.md"), [0xff, 0xfe, b'h', b'i']).unwrap();

    let report = store.load(d).unwrap();
    assert_eq!(report.warnings.len(), 2, "{:?}", report.warnings);
    assert_eq!(report.tickets.len(), 1);
    assert_eq!(report.tickets[0].id.as_str(), "ABC-2");
    assert_eq!(report.diagrams.len(), 1);
    assert!(report.overview.ends_with("hi"));
}

#[test]
fn export_includes_every_section_and_strips_ansi() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::new(tmp.path());
    let d = date("2026-10-27");
    let dir = store.create_report(d, &ReportManifest::default()).unwrap();
    fs::write(dir.join("diagrams/01-a.txt"), "\u{1b}[31mBUG\u{1b}[0m").unwrap();
    store
        .upsert_ticket(d, &ticket("ABC-1", "Done"), false)
        .unwrap();
    let md = export_markdown(&store.load(d).unwrap());
    assert!(md.starts_with("# Standup 2026-10-27"));
    assert!(md.contains("## ABC-1 — title of ABC-1"));
    assert!(md.contains("- ticket: https://tracker.example.com/issue/ABC-1"));
    assert!(md.contains("# 01-a\nBUG\n"));
    assert!(!md.contains('\u{1b}'));
}

#[test]
fn discovery_walks_up_to_the_nearest_standups_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let nested = tmp.path().join("a/b");
    fs::create_dir_all(&nested).unwrap();
    assert_eq!(discover_root(&nested), nested);
    fs::create_dir(tmp.path().join("standups")).unwrap();
    assert_eq!(discover_root(&nested), tmp.path());
}
