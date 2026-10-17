//! Tests for the version picker's state machine (no terminal).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::*;

fn v(s: &str) -> Version {
    s.parse().unwrap()
}

fn picker() -> Picker {
    let versions = [v("0.30.0"), v("0.29.1"), v("0.29.0"), v("0.28.0")];
    Picker::new(items(
        &versions,
        &[v("0.29.0"), v("0.28.0")],
        Some(v("0.29.0")),
    ))
}

#[test]
fn enter_on_an_empty_query_picks_the_latest() {
    assert_eq!(picker().handle(Key::Enter), Step::Pick(v("0.30.0")));
}

#[test]
fn esc_aborts() {
    assert_eq!(picker().handle(Key::Esc), Step::Abort);
}

#[test]
fn arrows_move_and_clamp() {
    let mut p = picker();
    p.handle(Key::Up);
    for _ in 0..10 {
        p.handle(Key::Down);
    }
    assert_eq!(p.handle(Key::Enter), Step::Pick(v("0.28.0")));
}

#[test]
fn typing_filters_and_resets_the_selection() {
    let mut p = picker();
    p.handle(Key::Down);
    for c in "29.1".chars() {
        p.handle(Key::Char(c));
    }
    assert_eq!(p.query(), "29.1");
    assert_eq!(p.visible()[0].version, v("0.29.1"));
    assert_eq!(p.handle(Key::Enter), Step::Pick(v("0.29.1")));
}

#[test]
fn no_match_makes_enter_a_no_op_and_backspace_recovers() {
    let mut p = picker();
    p.handle(Key::Char('x'));
    assert_eq!(p.visible(), []);
    assert_eq!(p.handle(Key::Enter), Step::Continue);
    p.handle(Key::Backspace);
    assert_eq!(p.visible().len(), 4);
}

#[test]
fn render_marks_latest_installed_and_current() {
    let lines = picker().render("rca", 20);
    assert!(lines[0].starts_with("rca — type to filter"));
    assert_eq!(lines[2], "▸   0.30.0  latest");
    assert_eq!(lines[4], "  ● 0.29.0  current");
    assert_eq!(lines[5], "  ● 0.28.0");
}

#[test]
fn render_scrolls_to_keep_the_selection_visible() {
    let mut p = picker();
    for _ in 0..3 {
        p.handle(Key::Down);
    }
    let lines = p.render("rca", 4); // header + query + 2 rows
    assert_eq!(lines.len(), 4);
    assert!(lines[3].starts_with("▸ ● 0.28.0"), "{lines:?}");
}
