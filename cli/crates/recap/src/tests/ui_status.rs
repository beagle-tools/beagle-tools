//! TUI tests: live PR status, list grouping and the delta diff view.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::tests::{fixture, key};
use super::*;
use crate::model::PrState;
use crate::wrap;
use crossterm::event::KeyCode;

#[test]
fn poller_results_recategorize_and_badge_the_list() {
    use crate::poller::{Checks, PrStatus, Review};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use std::collections::HashMap;
    let (_tmp, store) = fixture();
    let mut app = App::new(store.clone(), None).unwrap();
    assert_eq!(
        app.rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
        ["newer-2", "older-1"]
    );

    // older-1 turns out to be a draft, newer-2 is approved with failing CI.
    let mut batch = HashMap::new();
    batch.insert(
        "https://github.com/o/older/pull/1".to_owned(),
        PrStatus {
            state: PrState::Draft,
            review: Some(Review::Required),
            checks: Checks::default(),
            updated: None,
            merged_at: None,
        },
    );
    batch.insert(
        "https://github.com/o/newer/pull/2".to_owned(),
        PrStatus {
            state: PrState::Open,
            review: Some(Review::Approved),
            checks: Checks {
                passed: 3,
                failed: 1,
                pending: 0,
            },
            updated: None,
            merged_at: None,
        },
    );
    app.apply_statuses(batch);
    assert_eq!(
        app.rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
        ["older-1", "newer-2"],
        "drafts first"
    );
    assert_eq!(
        app.selected_row().unwrap().id,
        "newer-2",
        "selection follows the PR"
    );
    assert_eq!(
        app.status.as_deref(),
        Some("PR status: older#1 open → draft")
    );
    assert_eq!(
        store
            .load(&RecapId::new("older-1").unwrap())
            .unwrap()
            .manifest
            .state,
        PrState::Draft
    );

    let mut t = Terminal::new(TestBackend::new(140, 20)).unwrap();
    t.draw(|f| view::draw(f, &mut app)).unwrap();
    let screen: String = t
        .backend()
        .buffer()
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect();
    assert!(screen.contains("✓ approved"), "header badge");
    assert!(screen.contains("CI ✗ 1 failing"), "header CI");
    assert!(screen.contains("older#1 ○"), "sidebar badge");
    assert!(screen.contains("── DRAFTS · 1"), "draft heading");
    assert!(screen.contains("── OPEN · 1"), "open heading");
}

#[test]
fn list_is_partitioned_by_state_with_headings() {
    let row = |id: &str, state: PrState| RecapSummary {
        id: id.to_owned(),
        repo: "o/r".to_owned(),
        number: 1,
        title: String::new(),
        state,
        ticket: None,
        created: None,
        url: String::new(),
    };
    // Deliberately out of order: grouping must not rely on the sort.
    let rows = [
        row("m1", PrState::Merged),
        row("d1", PrState::Draft),
        row("o1", PrState::Open),
        row("d2", PrState::Draft),
        row("m2", PrState::Merged),
    ];
    assert_eq!(
        list_entries(&rows),
        [
            ListEntry::Header(PrState::Draft, 2),
            ListEntry::Row(1),
            ListEntry::Row(3),
            ListEntry::Header(PrState::Open, 1),
            ListEntry::Row(2),
            ListEntry::Header(PrState::Merged, 2),
            ListEntry::Row(0),
            ListEntry::Row(4),
        ]
    );
    assert_eq!(list_entries(&[]), []);
}

#[test]
fn diff_tab_uses_delta_side_by_side_when_installed() {
    if !crate::diff::delta_available() {
        return;
    }
    let (_tmp, store) = fixture();
    let mut app = App::new(store, None).unwrap();
    app.view.viewport = 20;
    key(&mut app, KeyCode::Char('2'));
    assert!(app.view.doc.is_empty(), "waits for the width");
    app.layout(120);
    let text: Vec<String> = app.view.doc.iter().map(wrap::plain_text).collect();
    assert!(
        text.iter()
            .any(|l| l.contains("added") && l.matches('│').count() >= 3),
        "side by side"
    );
    assert_eq!(app.file_starts.len(), 2);
    key(&mut app, KeyCode::Char('n'));
    assert_eq!(
        wrap::plain_text(&app.view.doc[app.view.owners[app.view.cursor]]),
        "a.txt"
    );
    app.layout(90); // resize re-renders at the new width
    assert_eq!(app.delta_width, 90);
}
