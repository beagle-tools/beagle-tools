//! Tests for archiving and unarchiving workspaces (`store::mutate`).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use crate::model::{SectionKind, Severity, Status};
use crate::store::testutil::{test_id, test_meta};
use crate::store::Store;

#[test]
fn archive_moves_a_finished_workspace_and_listing_splits() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let store = Store::open(tmp.path()).expect("open store");
    let id = test_id("done-rca");
    store
        .scaffold(&id, &test_meta("Done", Severity::Low))
        .expect("scaffold");
    store.set_status(&id, Status::Finished).expect("finish");

    let dest = store.archive(&id, false).expect("archive");
    assert!(dest.ends_with("archive/done-rca"), "moved to {dest:?}");
    assert!(dest.join("rca.toml").exists());

    let active = store.list().expect("list");
    assert!(
        active.summaries.is_empty(),
        "archived leaves the active list"
    );
    assert!(
        active.broken.is_empty(),
        "archive/ must not be reported as a broken workspace"
    );
    let archived = store.list_archived().expect("archived");
    assert_eq!(archived.summaries.len(), 1);
    assert!(archived.summaries[0].archived);

    let all = store.list_all().expect("all");
    assert_eq!(all.summaries.len(), 1);
}

#[test]
fn archive_refuses_unfinished_unless_forced_and_never_twice() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let store = Store::open(tmp.path()).expect("open store");
    let id = test_id("live-rca");
    store
        .scaffold(&id, &test_meta("Live", Severity::High))
        .expect("scaffold");

    let err = store.archive(&id, false).expect_err("investigating");
    assert!(err.to_string().contains("finished"), "explains: {err}");

    store.archive(&id, true).expect("force overrides");
    let err = store.archive(&id, true).expect_err("already archived");
    assert!(err.to_string().contains("already archived"), "got: {err}");
}

#[test]
fn archived_workspaces_read_and_export_transparently() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let store = Store::open(tmp.path()).expect("open store");
    let id = test_id("old-rca");
    store
        .scaffold(&id, &test_meta("Old", Severity::Low))
        .expect("scaffold");
    store.set_status(&id, Status::Finished).expect("finish");
    store.archive(&id, false).expect("archive");

    let summary = store
        .read_section(&id, SectionKind::Summary)
        .expect("read")
        .expect("present");
    assert!(summary.contains("Old"), "sections resolve into archive/");
    let doc = store.export_markdown(&id).expect("export");
    assert!(doc.contains("title: \"Old\""));
    store.append_log(&id, "post-archive note").expect("log");
}

#[test]
fn scaffold_refuses_the_archive_slug_and_archived_collisions() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let store = Store::open(tmp.path()).expect("open store");
    let reserved = test_id("archive");
    let err = store
        .scaffold(&reserved, &test_meta("Nope", Severity::Low))
        .expect_err("reserved");
    assert!(err.to_string().contains("reserved"), "got: {err}");

    let id = test_id("gone-rca");
    store
        .scaffold(&id, &test_meta("Gone", Severity::Low))
        .expect("scaffold");
    store.archive(&id, true).expect("archive");
    store
        .scaffold(&id, &test_meta("Again", Severity::Low))
        .expect_err("archived slug still occupied");
}

#[test]
fn unarchive_restores_a_workspace_and_round_trips() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let store = Store::open(tmp.path()).expect("open store");
    let id = test_id("back-rca");
    store
        .scaffold(&id, &test_meta("Back", Severity::Low))
        .expect("scaffold");
    store.archive(&id, true).expect("archive");

    let dest = store.unarchive(&id).expect("unarchive");
    assert!(dest.ends_with("rcas/back-rca"));
    let listing = store.list().expect("list");
    assert_eq!(listing.summaries.len(), 1);
    assert!(!listing.summaries[0].archived);

    let err = store.unarchive(&id).expect_err("not archived anymore");
    assert!(err.to_string().contains("not archived"), "got: {err}");
    let missing = test_id("never-was");
    let err = store.unarchive(&missing).expect_err("missing");
    assert!(err.to_string().contains("no workspace"), "got: {err}");
}
