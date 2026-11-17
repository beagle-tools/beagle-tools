//! Tests for `docview`.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::*;

fn view_of(lines: usize, viewport: u16) -> DocView {
    let mut view = DocView::new();
    view.set_doc(
        (0..lines)
            .map(|i| DocLine::prose(Line::raw(format!("line {i}"))))
            .collect(),
    );
    view.viewport = viewport;
    view.layout(40);
    view
}

#[test]
fn cursor_moves_and_the_view_follows() {
    let mut view = view_of(50, 10);
    view.move_cursor(15);
    assert_eq!(view.cursor, 15);
    assert_eq!(view.scroll, 6);
    view.move_cursor(isize::MAX);
    assert_eq!(view.cursor, 49);
    assert_eq!(view.scroll, view.max_scroll());
    view.move_cursor(isize::MIN);
    assert_eq!((view.cursor, view.scroll), (0, 0));
}

#[test]
fn nav_keys_page_and_ignore_others() {
    let mut view = view_of(50, 10);
    assert!(view.nav_key(KeyCode::Char(' '), false));
    assert_eq!(view.cursor, 8);
    assert!(view.nav_key(KeyCode::Char('d'), true));
    assert_eq!(view.cursor, 13);
    assert!(view.nav_key(KeyCode::Char('G'), false));
    assert_eq!(view.cursor, 49);
    assert!(!view.nav_key(KeyCode::Char('j'), false));
    assert!(!view.nav_key(KeyCode::Char('d'), false));
}

#[test]
fn pan_is_bounded_by_the_widest_line() {
    let mut view = DocView::new();
    view.set_doc(vec![DocLine::pre(Line::raw("x".repeat(100)))]);
    view.layout(40);
    view.content_width = 40;
    for _ in 0..20 {
        view.nav_key(KeyCode::Char('l'), false);
    }
    assert_eq!(view.pan, 60);
    view.nav_key(KeyCode::Char('h'), false);
    assert_eq!(view.pan, 52);
}

#[test]
fn visual_selection_covers_each_source_line_once() {
    let mut view = view_of(5, 10);
    view.move_cursor(1);
    assert!(view.toggle_visual().is_some());
    view.move_cursor(2);
    assert_eq!(view.selection(), (1, 3));
    assert_eq!(view.selection_text(), "line 1\nline 2\nline 3");
    assert!(view.toggle_visual().is_none());
    assert_eq!(view.selection(), (3, 3));
}

#[test]
fn relayout_keeps_the_cursor_on_its_source_line() {
    let mut view = DocView::new();
    view.set_doc(vec![
        DocLine::prose(Line::raw("word ".repeat(30))),
        DocLine::prose(Line::raw("target")),
    ]);
    view.layout(200);
    view.move_cursor(1);
    view.layout(20);
    assert_eq!(view.owners[view.cursor], 1);
}

#[test]
fn entering_puts_an_offscreen_cursor_on_the_first_visible_row() {
    let mut view = view_of(50, 10);
    view.scroll = 20;
    view.cursor = 2;
    view.anchor = Some(1);
    view.enter();
    assert_eq!((view.cursor, view.anchor), (20, None));
}

#[test]
fn visible_is_the_viewport_slice() {
    let mut view = view_of(50, 3);
    view.scroll = 4;
    let rows = view.visible();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].spans[0].content, "line 4");
}
