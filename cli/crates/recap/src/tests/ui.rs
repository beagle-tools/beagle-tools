//! State-transition tests for the TUI (no terminal).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::*;
use crate::model::{PrRef, PrState, RecapManifest};
use crate::wrap;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::fs;

pub(super) fn key(app: &mut App, code: KeyCode) {
    app.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
}

pub(super) fn manifest(repo: &str, number: u64, created: &str) -> RecapManifest {
    RecapManifest {
        repo: repo.to_owned(),
        number,
        title: format!("ABC-{number}: change {number}"),
        url: format!("https://github.com/{repo}/pull/{number}"),
        state: PrState::Open,
        author: None,
        branch: Some("feat".to_owned()),
        base: Some("main".to_owned()),
        ticket: Some(format!("ABC-{number}")),
        ticket_url: None,
        additions: 2,
        deletions: 1,
        files: 2,
        created: Some(created.to_owned()),
        updated: None,
        merged_at: None,
    }
}

pub(super) const DIFF: &str = "\
diff --git a/a.txt b/a.txt
--- a/a.txt
+++ b/a.txt
@@ -1 +1 @@
-old
+new
diff --git a/b.txt b/b.txt
--- a/b.txt
+++ b/b.txt
@@ -1 +1,2 @@
 same
+added
";

pub(super) fn id(s: &str) -> RecapId {
    RecapId::for_pr(&PrRef::parse(s).unwrap())
}

pub(super) fn fixture() -> (tempfile::TempDir, Store) {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::new(tmp.path());
    store
        .create(
            &id("o/older#1"),
            &manifest("o/older", 1, "2026-10-01T00:00:00Z"),
            DIFF,
        )
        .unwrap();
    let dir = store
        .create(
            &id("o/newer#2"),
            &manifest("o/newer", 2, "2026-10-28T00:00:00Z"),
            DIFF,
        )
        .unwrap();
    let long = (0..120)
        .map(|i| i.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(
        dir.join("summary.md"),
        format!("## TL;DR\nThe gist.\n\n{long}"),
    )
    .unwrap();
    (tmp, store)
}

pub(super) fn laid_out(store: Store) -> App {
    let mut app = App::new(store, None).unwrap();
    // The built-in renderer keeps these tests independent of `delta`.
    app.use_delta = false;
    app.view.viewport = 10;
    app.layout(80);
    app
}

#[test]
fn opens_on_the_newest_pr_summary() {
    let (_tmp, store) = fixture();
    let app = laid_out(store);
    assert_eq!(app.selected_row().unwrap().id, "newer-2");
    assert_eq!(app.tab, Tab::Summary);
    assert_eq!(wrap::plain_text(&app.view.doc[0]), "ABC-2: change 2");
}

#[test]
fn list_navigation_and_tabs() {
    let (_tmp, store) = fixture();
    let mut app = laid_out(store);
    key(&mut app, KeyCode::Char('j'));
    assert_eq!(app.selected_row().unwrap().id, "older-1");
    key(&mut app, KeyCode::Char('j'));
    assert_eq!(
        app.selected_row().unwrap().id,
        "older-1",
        "clamps at the end"
    );
    key(&mut app, KeyCode::Char('2'));
    assert_eq!(app.tab, Tab::Diff);
    assert_eq!(app.file_starts.len(), 2);
    key(&mut app, KeyCode::Esc); // back to the list
    key(&mut app, KeyCode::Char('k'));
    assert_eq!(app.selected_row().unwrap().id, "newer-2");
    assert_eq!(app.tab, Tab::Summary, "choosing a PR opens its Summary");
    let hint = app
        .view
        .doc
        .iter()
        .map(wrap::plain_text)
        .find(|l| l.starts_with("diff "))
        .unwrap();
    assert!(hint.contains("2 files changed"), "{hint}");
}

#[test]
fn tab_keys_focus_the_pane_so_j_scrolls_it() {
    let (_tmp, store) = fixture();
    let mut app = laid_out(store);
    assert_eq!(app.focus, Focus::List);
    let selected = app.selected_row().unwrap().id.clone();

    key(&mut app, KeyCode::Char('2'));
    assert_eq!(app.tab, Tab::Diff);
    assert_eq!(app.focus, Focus::Content, "2 jumps into the diff");
    app.layout(80);
    let cursor = app.view.cursor;
    key(&mut app, KeyCode::Char('j'));
    assert_eq!(app.view.cursor, cursor + 1, "j moves through the diff");
    assert_eq!(
        app.selected_row().unwrap().id,
        selected,
        "the PR selection stays put"
    );

    key(&mut app, KeyCode::Esc);
    key(&mut app, KeyCode::Char('1'));
    assert_eq!(app.tab, Tab::Summary);
    assert_eq!(app.focus, Focus::Content, "1 focuses the summary too");
}

#[test]
fn tabs_remember_their_position() {
    let (_tmp, store) = fixture();
    let mut app = laid_out(store);
    key(&mut app, KeyCode::Enter);
    for _ in 0..30 {
        key(&mut app, KeyCode::Char('j'));
    }
    let summary_pos = (app.view.scroll, app.view.cursor);
    key(&mut app, KeyCode::Char('2'));
    app.layout(80);
    assert_eq!(app.view.cursor, 0);
    key(&mut app, KeyCode::Char('1'));
    app.layout(80);
    assert_eq!((app.view.scroll, app.view.cursor), summary_pos);
}

#[test]
fn n_and_capital_n_walk_the_files() {
    let (_tmp, store) = fixture();
    let mut app = laid_out(store);
    key(&mut app, KeyCode::Char('n')); // from Summary: opens Diff
    app.layout(80);
    key(&mut app, KeyCode::Char('n'));
    let first = app.view.owners[app.view.cursor];
    assert_eq!(first, app.file_starts[0]);
    assert!(wrap::plain_text(&app.view.doc[first]).starts_with("━━ a.txt"));
    key(&mut app, KeyCode::Char('n'));
    assert_eq!(app.view.owners[app.view.cursor], app.file_starts[1]);
    key(&mut app, KeyCode::Char('n'));
    assert_eq!(app.status.as_deref(), Some("last file"));
    key(&mut app, KeyCode::Char('N'));
    assert_eq!(app.view.owners[app.view.cursor], app.file_starts[0]);
    assert_eq!(app.focus, Focus::Content);
}

#[test]
fn selection_copies_source_lines_from_the_diff() {
    let (_tmp, store) = fixture();
    let mut app = laid_out(store);
    key(&mut app, KeyCode::Char('2'));
    app.layout(80);
    key(&mut app, KeyCode::Char('n'));
    let start = app.view.cursor;
    app.view.cursor = start + 1; // the @@ line
    key(&mut app, KeyCode::Char('v'));
    key(&mut app, KeyCode::Char('j'));
    key(&mut app, KeyCode::Char('j'));
    assert_eq!(app.view.selection_text(), "@@ -1 +1 @@\n-old\n+new");
}

#[test]
fn open_ticket_without_url_explains_itself() {
    let (_tmp, store) = fixture();
    let mut app = laid_out(store);
    key(&mut app, KeyCode::Char('t'));
    assert!(app.status.as_deref().unwrap().contains("no ticket_url"));
}

#[test]
fn live_reload_keeps_the_reader_in_place_and_announces_new_prs() {
    let (_tmp, store) = fixture();
    let mut app = laid_out(store.clone());
    key(&mut app, KeyCode::Enter);
    for _ in 0..40 {
        key(&mut app, KeyCode::Char('j'));
    }
    let before = (app.view.cursor, app.view.scroll);
    store
        .create(
            &id("o/other#3"),
            &manifest("o/other", 3, "2026-09-01T00:00:00Z"),
            DIFF,
        )
        .unwrap();
    app.reload();
    app.layout(80);
    assert_eq!(app.selected_row().unwrap().id, "newer-2");
    assert_eq!((app.view.cursor, app.view.scroll), before);
    assert!(app
        .status
        .as_deref()
        .unwrap()
        .contains("new recap: o/other#3"));
}

#[test]
fn panning_is_bounded() {
    let (_tmp, store) = fixture();
    let mut app = laid_out(store);
    app.view.content_width = 80;
    for _ in 0..50 {
        key(&mut app, KeyCode::Right);
    }
    assert_eq!(app.view.pan, app.view.max_pan());
    assert!(app.view.pan < 100);
}

#[test]
fn draws_at_any_size_including_empty_store() {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    let (_tmp, store) = fixture();
    let mut app = App::new(store, None).unwrap();
    for (w, h) in [(120, 40), (40, 10), (1, 1), (0, 0)] {
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| view::draw(f, &mut app)).unwrap();
    }
    key(&mut app, KeyCode::Char('2'));
    key(&mut app, KeyCode::Char('?'));
    let mut t = Terminal::new(TestBackend::new(140, 40)).unwrap();
    t.draw(|f| view::draw(f, &mut app)).unwrap();
    let screen: String = t
        .backend()
        .buffer()
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect();
    assert!(screen.contains("newer#2"));
    assert!(screen.contains("2 Diff"));
    assert!(screen.contains("next / previous file"));

    let empty = tempfile::tempdir().unwrap();
    let mut app = App::new(Store::new(empty.path()), None).unwrap();
    key(&mut app, KeyCode::Char('j'));
    key(&mut app, KeyCode::Char('n'));
    key(&mut app, KeyCode::Char('o'));
    let mut t = Terminal::new(TestBackend::new(100, 20)).unwrap();
    t.draw(|f| view::draw(f, &mut app)).unwrap();
    let screen: String = t
        .backend()
        .buffer()
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect();
    assert!(screen.contains("No PR recaps yet"));
}

/// An app whose GitHub actions are recorded, never run.
pub(super) fn offline(store: Store) -> App {
    let mut app = laid_out(store);
    app.actions_enabled = false;
    app
}
