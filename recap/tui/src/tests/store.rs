//! Tests for `store`.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::*;
use crate::model::{PrRef, PrState};

pub(crate) fn manifest(repo: &str, number: u64, created: &str) -> RecapManifest {
    RecapManifest {
        repo: repo.to_owned(),
        number,
        title: format!("ABC-{number}: change {number}"),
        url: format!("https://github.com/{repo}/pull/{number}"),
        state: PrState::Open,
        author: Some("me".to_owned()),
        branch: Some("feature".to_owned()),
        base: Some("main".to_owned()),
        ticket: Some(format!("ABC-{number}")),
        ticket_url: None,
        additions: 3,
        deletions: 1,
        files: 1,
        created: Some(created.to_owned()),
        updated: None,
        merged_at: None,
    }
}

pub(crate) const DIFF: &str =
    "diff --git a/a.txt b/a.txt\n--- a/a.txt\n+++ b/a.txt\n@@ -1 +1 @@\n-old\n+new\n";

fn id(s: &str) -> RecapId {
    RecapId::for_pr(&PrRef::parse(s).unwrap())
}

#[test]
fn missing_store_is_empty_and_discovery_walks_up() {
    let tmp = tempfile::tempdir().unwrap();
    assert_eq!(
        Store::new(tmp.path()).list().unwrap().0,
        Vec::<RecapSummary>::new()
    );
    let nested = tmp.path().join("a/b");
    fs::create_dir_all(&nested).unwrap();
    assert_eq!(discover_root(&nested), nested);
    fs::create_dir(tmp.path().join("recaps")).unwrap();
    assert_eq!(discover_root(&nested), tmp.path());
}

#[test]
fn create_list_load_round_trip_newest_first() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::new(tmp.path());
    store
        .create(
            &id("o/gadgets#1"),
            &manifest("o/gadgets", 1, "2026-10-01T00:00:00Z"),
            DIFF,
        )
        .unwrap();
    store
        .create(
            &id("o/widgets#2"),
            &manifest("o/widgets", 2, "2026-10-28T00:00:00Z"),
            DIFF,
        )
        .unwrap();
    let (rows, warnings) = store.list().unwrap();
    assert_eq!(warnings, Vec::<String>::new());
    assert_eq!(
        rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
        ["widgets-2", "gadgets-1"]
    );
    let r = store.load(&id("o/widgets#2")).unwrap();
    assert_eq!(r.manifest.number, 2);
    assert!(r.summary.contains("## How to review it"));
    assert_eq!(r.diff, DIFF);
    assert!(store.create(&id("o/widgets#2"), &r.manifest, DIFF).is_err());
}

#[test]
fn refresh_keeps_the_write_up_and_ticket_url() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::new(tmp.path());
    let rid = id("o/r#5");
    let mut m = manifest("o/r", 5, "2026-10-28T00:00:00Z");
    m.ticket_url = Some("https://tracker.example.com/issue/ABC-5".to_owned());
    let dir = store.create(&rid, &m, DIFF).unwrap();
    fs::write(dir.join("summary.md"), "hand written").unwrap();
    let mut newer = manifest("o/r", 5, "2026-10-28T00:00:00Z");
    newer.state = PrState::Merged;
    store.refresh(&rid, &newer, "diff --git a/b b/b\n").unwrap();
    let r = store.load(&rid).unwrap();
    assert_eq!(r.summary, "hand written");
    assert_eq!(r.manifest.state, PrState::Merged);
    assert_eq!(
        r.manifest.ticket_url.as_deref(),
        Some("https://tracker.example.com/issue/ABC-5")
    );
    assert!(r.diff.starts_with("diff --git a/b"));
    assert!(store.refresh(&id("o/r#6"), &newer, "").is_err());
}

#[test]
fn broken_manifests_are_reported_not_fatal() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::new(tmp.path());
    store
        .create(
            &id("o/r#1"),
            &manifest("o/r", 1, "2026-10-28T00:00:00Z"),
            DIFF,
        )
        .unwrap();
    let bad = tmp.path().join("recaps/r-2");
    fs::create_dir_all(&bad).unwrap();
    fs::write(bad.join("recap.toml"), "nope = true").unwrap();
    fs::create_dir_all(tmp.path().join("recaps/NotAnId")).unwrap();
    let (rows, warnings) = store.list().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(warnings.len(), 1);
    assert!(store.load(&RecapId::new("r-2").unwrap()).is_err());
}

#[test]
fn export_has_metadata_summary_and_file_stats() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::new(tmp.path());
    let rid = id("o/r#7");
    let dir = store
        .create(&rid, &manifest("o/r", 7, "2026-10-28T00:00:00Z"), DIFF)
        .unwrap();
    fs::write(dir.join("diagrams/01-x.txt"), "\u{1b}[31mBUG\u{1b}[0m").unwrap();
    let r = store.load(&rid).unwrap();
    let md = export_markdown(&r, false);
    assert!(md.starts_with("# o/r#7 — ABC-7: change 7"));
    assert!(md.contains("- ticket: ABC-7"));
    assert!(md.contains("+1     -1     a.txt"));
    assert!(md.contains("# 01-x\nBUG\n"));
    assert!(!md.contains("## Diff"));
    assert!(export_markdown(&r, true).contains("```diff\ndiff --git"));
}

#[test]
fn list_groups_drafts_then_open_then_merged() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::new(tmp.path());
    for (n, state, created) in [
        (1, PrState::Merged, "2026-10-28T00:00:00Z"),
        (2, PrState::Open, "2026-10-01T00:00:00Z"),
        (3, PrState::Draft, "2026-09-01T00:00:00Z"),
        (4, PrState::Open, "2026-10-20T00:00:00Z"),
        (5, PrState::Closed, "2026-10-29T00:00:00Z"),
        (6, PrState::Draft, "2026-10-25T00:00:00Z"),
    ] {
        let mut m = manifest("o/r", n, created);
        m.state = state;
        store.create(&id(&format!("o/r#{n}")), &m, DIFF).unwrap();
    }
    let ids: Vec<String> = store.list().unwrap().0.into_iter().map(|r| r.id).collect();
    assert_eq!(ids, ["r-6", "r-3", "r-4", "r-2", "r-1", "r-5"]);
}

#[test]
fn set_state_rewrites_only_the_manifest() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::new(tmp.path());
    let rid = id("o/r#8");
    let dir = store
        .create(&rid, &manifest("o/r", 8, "2026-10-28T00:00:00Z"), DIFF)
        .unwrap();
    fs::write(dir.join("summary.md"), "mine").unwrap();
    assert!(store
        .set_state(&rid, PrState::Merged, Some("2026-10-29T00:00:00Z"))
        .unwrap());
    assert!(
        !store
            .set_state(&rid, PrState::Merged, Some("2026-10-29T00:00:00Z"))
            .unwrap(),
        "no-op"
    );
    let r = store.load(&rid).unwrap();
    assert_eq!(r.manifest.state, PrState::Merged);
    assert_eq!(r.manifest.updated.as_deref(), Some("2026-10-29T00:00:00Z"));
    assert_eq!(r.summary, "mine");
    assert_eq!(r.diff, DIFF);
    assert!(store.set_state(&id("o/r#9"), PrState::Open, None).is_err());
}

#[test]
fn archive_moves_a_recap_off_the_active_list_and_back() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::new(tmp.path());
    let rid = id("o/r#11");
    let dir = store
        .create(&rid, &manifest("o/r", 11, "2026-10-28T00:00:00Z"), DIFF)
        .unwrap();
    fs::write(dir.join("summary.md"), "kept").unwrap();
    store
        .create(
            &id("o/r#12"),
            &manifest("o/r", 12, "2026-10-28T00:00:00Z"),
            DIFF,
        )
        .unwrap();

    store.archive(&rid).unwrap();
    let active: Vec<String> = store.list().unwrap().0.into_iter().map(|r| r.id).collect();
    assert_eq!(
        active,
        ["r-12"],
        "archived recap leaves the list; _archive isn't a recap"
    );
    let archived: Vec<String> = store
        .list_shelf(Shelf::Archived)
        .unwrap()
        .0
        .into_iter()
        .map(|r| r.id)
        .collect();
    assert_eq!(archived, ["r-11"]);
    assert_eq!(
        store.load_from(Shelf::Archived, &rid).unwrap().summary,
        "kept"
    );
    assert!(store.load(&rid).is_err());
    assert_eq!(
        store.load_any(&rid).unwrap().summary,
        "kept",
        "read-only lookups see the archive"
    );
    assert!(store.archive(&rid).is_err(), "can't archive twice");

    store.unarchive(&rid).unwrap();
    assert_eq!(store.load(&rid).unwrap().summary, "kept");
    assert_eq!(
        store.list_shelf(Shelf::Archived).unwrap().0,
        Vec::<RecapSummary>::new()
    );
    assert!(store.unarchive(&rid).is_err());
}

#[test]
fn merged_recaps_archive_24h_after_merging_and_stay_readable() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::new(tmp.path());
    let at = |ts: &str| crate::model::unix_seconds(ts).unwrap();
    let mk = |n: u64, state: PrState, merged_at: Option<&str>, updated: Option<&str>| {
        let mut m = manifest("o/r", n, "2026-10-20T00:00:00Z");
        m.state = state;
        m.merged_at = merged_at.map(str::to_owned);
        m.updated = updated.map(str::to_owned);
        store.create(&id(&format!("o/r#{n}")), &m, DIFF).unwrap();
    };
    mk(1, PrState::Merged, Some("2026-10-27T10:00:00Z"), None); // 26h before "now": archive
    mk(2, PrState::Merged, Some("2026-10-28T10:00:00Z"), None); // 2h before: keep
    mk(3, PrState::Merged, None, Some("2026-10-26T00:00:00Z")); // no merged_at: falls back to updated
    mk(4, PrState::Merged, None, None); // no time at all: keep
    mk(5, PrState::Open, None, Some("2026-01-01T00:00:00Z")); // not merged: keep
    let now = at("2026-10-28T12:00:00Z");

    let mut archived = store
        .archive_merged_older_than(now, AUTO_ARCHIVE_AFTER_SECS)
        .unwrap();
    archived.sort();
    assert_eq!(archived, ["r-1", "r-3"]);
    let mut active: Vec<String> = store.list().unwrap().0.into_iter().map(|r| r.id).collect();
    active.sort();
    assert_eq!(active, ["r-2", "r-4", "r-5"]);
    assert_eq!(
        store
            .load_from(Shelf::Archived, &id("o/r#1"))
            .unwrap()
            .summary
            .lines()
            .next(),
        Some("## TL;DR")
    );
    assert_eq!(
        store
            .archive_merged_older_than(now, AUTO_ARCHIVE_AFTER_SECS)
            .unwrap(),
        Vec::<String>::new(),
        "idempotent"
    );

    // Exactly 24h counts.
    assert_eq!(
        store
            .archive_merged_older_than(at("2026-10-29T10:00:00Z"), AUTO_ARCHIVE_AFTER_SECS)
            .unwrap(),
        ["r-2"]
    );
}

#[test]
fn set_merged_at_records_once() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::new(tmp.path());
    let rid = id("o/r#20");
    store
        .create(&rid, &manifest("o/r", 20, "2026-10-28T00:00:00Z"), DIFF)
        .unwrap();
    assert!(store.set_merged_at(&rid, "2026-10-28T09:00:00Z").unwrap());
    assert!(!store.set_merged_at(&rid, "2026-10-28T09:00:00Z").unwrap());
    assert_eq!(
        store.load(&rid).unwrap().manifest.merged_at.as_deref(),
        Some("2026-10-28T09:00:00Z")
    );
}
