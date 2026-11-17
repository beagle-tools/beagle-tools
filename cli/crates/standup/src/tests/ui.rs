//! State-transition tests for the TUI (no terminal).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::*;
use crate::model::{ReportManifest, TicketManifest};
use crate::wrap;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::fs;

fn key(app: &mut App, code: KeyCode) {
    app.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
}

fn date(s: &str) -> ReportDate {
    ReportDate::parse(s).unwrap()
}

fn ticket(id: &str, state: &str) -> TicketManifest {
    TicketManifest {
        id: id.to_owned(),
        title: format!("{id} title"),
        url: None,
        state: Some(state.to_owned()),
        systems: Vec::new(),
        prs: Vec::new(),
    }
}

fn fixture() -> (tempfile::TempDir, Store) {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::new(tmp.path());
    for d in ["2026-10-23", "2026-10-27"] {
        store
            .create_report(date(d), &ReportManifest::default())
            .unwrap();
    }
    store
        .upsert_ticket(date("2026-10-27"), &ticket("ABC-1", "Done"), false)
        .unwrap();
    store
        .upsert_ticket(date("2026-10-27"), &ticket("ABC-2", "In Progress"), false)
        .unwrap();
    let long = (0..200)
        .map(|i| i.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(
        store.report_dir(date("2026-10-27")).join("overview.md"),
        long,
    )
    .unwrap();
    (tmp, store)
}

#[test]
fn opens_on_the_newest_report_overview() {
    let (_tmp, store) = fixture();
    let app = App::new(store, None).unwrap();
    assert_eq!(app.current_date(), Some(date("2026-10-27")));
    assert_eq!(app.selected, 0);
    assert_eq!(app.item_count(), 3);
    assert!(app.selected_ticket().is_none());
}

#[test]
fn list_navigation_selects_tickets_in_activity_order() {
    let (_tmp, store) = fixture();
    let mut app = App::new(store, None).unwrap();
    key(&mut app, KeyCode::Char('j'));
    assert_eq!(app.selected_ticket().unwrap().id.as_str(), "ABC-2");
    key(&mut app, KeyCode::Char('j'));
    key(&mut app, KeyCode::Char('j')); // clamps at the end
    assert_eq!(app.selected_ticket().unwrap().id.as_str(), "ABC-1");
    key(&mut app, KeyCode::Char('k'));
    key(&mut app, KeyCode::Char('k'));
    key(&mut app, KeyCode::Char('k'));
    assert_eq!(app.selected, 0);
}

#[test]
fn content_cursor_moves_and_the_view_follows() {
    let (_tmp, store) = fixture();
    let mut app = App::new(store, None).unwrap();
    app.view.viewport = 10;
    app.view.layout(80);
    key(&mut app, KeyCode::Tab);
    assert_eq!(app.focus, Focus::Content);
    key(&mut app, KeyCode::Char('j'));
    assert_eq!(app.view.cursor, 1);
    assert_eq!(
        app.view.scroll, 0,
        "the view only moves once the cursor leaves it"
    );
    assert_eq!(app.selected, 0, "j moves the cursor, not the list");
    for _ in 0..12 {
        key(&mut app, KeyCode::Char('j'));
    }
    assert_eq!(app.view.cursor, 13);
    assert_eq!(app.view.scroll, 4, "cursor stays on the last visible row");
    key(&mut app, KeyCode::Char('G'));
    assert_eq!(app.view.cursor, app.view.laid.len() - 1);
    assert_eq!(app.view.scroll, app.view.max_scroll());
    key(&mut app, KeyCode::Char('g'));
    assert_eq!((app.view.cursor, app.view.scroll), (0, 0));
    key(&mut app, KeyCode::Esc);
    assert_eq!(app.focus, Focus::List);
}

#[test]
fn visual_selection_covers_whole_source_lines() {
    let (_tmp, store) = fixture();
    let mut app = App::new(store, None).unwrap();
    app.view.viewport = 10;
    app.view.layout(80);
    key(&mut app, KeyCode::Tab);
    // Find the overview's "0" .. "2" lines (the fixture writes 0..199).
    let start = app
        .view
        .doc
        .iter()
        .position(|l| wrap::plain_text(l) == "0")
        .unwrap();
    app.view.cursor = app.view.owners.iter().position(|o| *o == start).unwrap();
    key(&mut app, KeyCode::Char('v'));
    key(&mut app, KeyCode::Char('j'));
    key(&mut app, KeyCode::Char('j'));
    assert_eq!(app.view.selection_text(), "0\n1\n2");
    key(&mut app, KeyCode::Esc);
    assert!(app.view.anchor.is_none(), "esc ends the selection first");
    assert_eq!(app.focus, Focus::Content);
    assert_eq!(
        app.view.selection_text(),
        "2",
        "no selection: just the cursor line"
    );
}

#[test]
fn panning_stops_at_the_widest_line() {
    let (_tmp, store) = fixture();
    let mut app = App::new(store, None).unwrap();
    app.view.layout(80);
    app.view.content_width = 80;
    for _ in 0..50 {
        key(&mut app, KeyCode::Right);
    }
    assert_eq!(app.view.pan, 0, "nothing wider than the pane: no panning");
    let wide = store_with_wide_diagram();
    let mut app = App::new(wide.1, None).unwrap();
    app.view.layout(80);
    app.view.content_width = 80;
    for _ in 0..50 {
        key(&mut app, KeyCode::Right);
    }
    assert_eq!(
        app.view.pan,
        120 - 80,
        "stops once the widest line is fully in view"
    );
    key(&mut app, KeyCode::Left);
    assert_eq!(app.view.pan, 32);
}

fn store_with_wide_diagram() -> (tempfile::TempDir, Store) {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::new(tmp.path());
    let dir = store
        .create_report(date("2026-10-27"), &ReportManifest::default())
        .unwrap();
    fs::write(dir.join("diagrams/01-wide.txt"), "x".repeat(120)).unwrap();
    (tmp, store)
}

#[test]
fn ticket_header_does_not_repeat_the_id() {
    let (_tmp, store) = fixture();
    let mut app = App::new(store, None).unwrap();
    key(&mut app, KeyCode::Char('j'));
    let first = wrap::plain_text(&app.view.doc[0]);
    assert_eq!(
        first, "ABC-2 title",
        "the title only — the border shows the id"
    );
}

#[test]
fn brackets_walk_between_reports() {
    let (_tmp, store) = fixture();
    let mut app = App::new(store, None).unwrap();
    key(&mut app, KeyCode::Char(']'));
    assert!(app.status.as_deref().unwrap().contains("newest"));
    key(&mut app, KeyCode::Char('['));
    assert_eq!(app.current_date(), Some(date("2026-10-23")));
    assert_eq!(app.item_count(), 1);
    key(&mut app, KeyCode::Char('['));
    assert!(app.status.as_deref().unwrap().contains("older"));
}

#[test]
fn open_without_a_ticket_or_url_explains_itself() {
    let (_tmp, store) = fixture();
    let mut app = App::new(store, None).unwrap();
    key(&mut app, KeyCode::Char('o'));
    assert!(app.status.as_deref().unwrap().contains("select a ticket"));
    key(&mut app, KeyCode::Char('j'));
    key(&mut app, KeyCode::Char('o'));
    assert!(app.status.as_deref().unwrap().contains("no url"));
}

#[test]
fn reload_keeps_the_selected_ticket_and_follows_new_reports() {
    let (_tmp, store) = fixture();
    let mut app = App::new(store.clone(), None).unwrap();
    key(&mut app, KeyCode::Char('j'));
    key(&mut app, KeyCode::Char('j')); // ABC-1
    store
        .upsert_ticket(date("2026-10-27"), &ticket("ABC-0", "In Review"), false)
        .unwrap();
    app.reload();
    assert_eq!(app.selected_ticket().unwrap().id.as_str(), "ABC-1");

    store
        .create_report(date("2026-10-28"), &ReportManifest::default())
        .unwrap();
    app.reload();
    assert_eq!(app.current_date(), Some(date("2026-10-28")));
    assert!(app.status.as_deref().unwrap().contains("new report"));
}

#[test]
fn empty_store_and_help_and_quit() {
    let tmp = tempfile::tempdir().unwrap();
    let mut app = App::new(Store::new(tmp.path().join("none")), None).unwrap();
    assert!(app.current_date().is_none());
    key(&mut app, KeyCode::Char('j'));
    key(&mut app, KeyCode::Char('?'));
    assert!(app.help);
    key(&mut app, KeyCode::Char('x'));
    assert!(!app.help && !app.should_quit());
    key(&mut app, KeyCode::Char('q'));
    assert!(app.should_quit());
}

#[test]
fn draws_without_panicking_at_any_size() {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    let (_tmp, store) = fixture();
    let mut app = App::new(store, None).unwrap();
    for (w, h) in [(120, 40), (40, 10), (1, 1), (0, 0)] {
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        terminal.draw(|f| view::draw(f, &mut app)).unwrap();
    }
    key(&mut app, KeyCode::Char('d'));
    for (w, h) in [(120, 40), (10, 4), (0, 0)] {
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        terminal.draw(|f| view::draw(f, &mut app)).unwrap();
    }
    key(&mut app, KeyCode::Esc);
    key(&mut app, KeyCode::Char('?'));
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
    terminal.draw(|f| view::draw(f, &mut app)).unwrap();
    let screen: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect();
    assert!(screen.contains("Overview"));
    assert!(screen.contains("ABC-2"));
    assert!(screen.contains("open the ticket in the tracker"));
}

#[test]
fn picker_lists_reports_and_opens_the_chosen_day() {
    let (_tmp, store) = fixture();
    let mut app = App::new(store, None).unwrap();
    key(&mut app, KeyCode::Char('j')); // a ticket on the newest report
    key(&mut app, KeyCode::Char('d'));
    let picker = app.picker.as_ref().unwrap();
    assert_eq!(picker.rows.len(), 2);
    assert_eq!(picker.selected, 0, "starts on the report on screen");
    key(&mut app, KeyCode::Char('j'));
    key(&mut app, KeyCode::Char('j')); // clamps
    key(&mut app, KeyCode::Enter);
    assert!(app.picker.is_none());
    assert_eq!(app.current_date(), Some(date("2026-10-23")));
    assert_eq!(app.selected, 0, "lands on the overview");
    key(&mut app, KeyCode::Char('d'));
    key(&mut app, KeyCode::Esc);
    assert!(app.picker.is_none());
    assert_eq!(
        app.current_date(),
        Some(date("2026-10-23")),
        "esc changes nothing"
    );
}

#[test]
fn live_reload_keeps_the_reader_in_place() {
    let (_tmp, store) = fixture();
    let mut app = App::new(store.clone(), None).unwrap();
    app.view.viewport = 10;
    app.view.layout(80);
    key(&mut app, KeyCode::Tab);
    for _ in 0..40 {
        key(&mut app, KeyCode::Char('j'));
    }
    let before = (app.view.cursor, app.view.scroll);
    fs::write(
        store
            .report_dir(date("2026-10-27"))
            .join("diagrams/01-new.txt"),
        "box",
    )
    .unwrap();
    app.reload();
    app.view.layout(80);
    assert_eq!((app.view.cursor, app.view.scroll), before);
}
