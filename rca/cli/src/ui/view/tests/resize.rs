//! `w` resize mode for the incident sidebar (`beagle_tui::pane_resize`).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use crossterm::event::KeyCode;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

use super::*;
use crate::ui::keys::Flow;
use crate::ui::testutil::{app_with, press};

fn screen(terminal: &mut Terminal<TestBackend>, app: &mut App) -> String {
    terminal.draw(|frame| draw(frame, app)).expect("draw");
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect()
}

#[test]
fn w_resizes_the_sidebar_within_its_limits() {
    let mut app = app_with(2);
    let mut terminal = Terminal::new(TestBackend::new(120, 30)).expect("terminal");
    screen(&mut terminal, &mut app);
    assert_eq!(app.side.width(120), SIDEBAR_WIDTH);

    press(&mut app, KeyCode::Char('w'));
    assert!(app.side.is_active());
    assert!(screen(&mut terminal, &mut app).contains(beagle_tui::pane_resize::HINT));
    press(&mut app, KeyCode::Right);
    assert_eq!(app.side.width(120), SIDEBAR_WIDTH + 2);
    // Rows pad to the live width, so the selection tint still fills them.
    screen(&mut terminal, &mut app);
    assert_eq!(app.mouse.sidebar.width, SIDEBAR_WIDTH + 2);

    for _ in 0..100 {
        press(&mut app, KeyCode::Left);
    }
    assert_eq!(app.side.width(120), app.side.limits().min);
    assert!(app.status.as_deref().unwrap().contains("narrowest"));
    // Keys that act elsewhere (delete, quit) do nothing while resizing.
    press(&mut app, KeyCode::Char('D'));
    assert!(app.side.is_active());
    press(&mut app, KeyCode::Char('0'));
    assert_eq!(app.side.width(120), SIDEBAR_WIDTH);
    assert_eq!(press(&mut app, KeyCode::Char('Q')), Flow::Continue);
    assert_eq!(press(&mut app, KeyCode::Char('q')), Flow::Continue);
    assert!(!app.side.is_active());

    press(&mut app, KeyCode::Char('w'));
    assert_eq!(press(&mut app, KeyCode::Esc), Flow::Continue);
    assert!(!app.side.is_active());
}

#[test]
fn w_on_a_hidden_sidebar_says_to_show_it_first() {
    let mut app = app_with(1);
    press(&mut app, KeyCode::Char('s'));
    assert!(app.sidebar_collapsed());
    press(&mut app, KeyCode::Char('w'));
    assert!(!app.side.is_active());
    assert!(app.status.as_deref().unwrap().contains("s shows it"));
}
