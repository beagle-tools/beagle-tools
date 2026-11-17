//! Tests for `widgets`.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::*;

#[test]
fn truncate_marks_a_cut_with_an_ellipsis() {
    assert_eq!(truncate("short", 10), "short");
    assert_eq!(truncate("a longer title", 8), "a longe…");
    assert_eq!(truncate("héllo wörld", 5), "héll…");
}

#[test]
fn centered_clamps_to_the_area() {
    let area = Rect::new(0, 0, 100, 40);
    assert_eq!(centered(area, 20, 10), Rect::new(40, 15, 20, 10));
    assert_eq!(centered(area, 200, 100), area);
}

#[test]
fn border_is_cyan_only_when_focused() {
    assert_eq!(border_style(true).fg, Some(Color::Cyan));
    assert_eq!(border_style(false).fg, Some(Color::DarkGray));
}
