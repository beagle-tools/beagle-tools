//! The standard beagle TUI keymap (`docs/CODING_STANDARDS.md` "Terminal
//! UIs"): one test pinning every binding rca shares with the other TUIs.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use crossterm::event::{KeyEvent, KeyModifiers};

use crate::ui::testutil::{app_with, press};

use super::*;

#[test]
fn rca_follows_the_standard_keymap() {
    let mut app = app_with(3);
    // Move: j/k, g/G in the list.
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(app.selected, 1);
    press(&mut app, KeyCode::Char('G'));
    assert_eq!(app.selected, 2);
    press(&mut app, KeyCode::Char('g'));
    assert_eq!(app.selected, 0);
    // Open / back: Enter or l opens, Esc or b goes back.
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.focus(), Focus::Content);
    press(&mut app, KeyCode::Esc);
    assert_eq!(app.focus(), Focus::List);
    press(&mut app, KeyCode::Char('l'));
    assert_eq!(app.focus(), Focus::Content);
    press(&mut app, KeyCode::Char('b'));
    assert_eq!(app.focus(), Focus::List);
    // Tabs: Tab / ] forward, [ back, digits jump.
    let first = app.tab();
    press(&mut app, KeyCode::Tab);
    assert_ne!(app.tab(), first);
    press(&mut app, KeyCode::Char('['));
    assert_eq!(app.tab(), first);
    press(&mut app, KeyCode::Char(']'));
    assert_ne!(app.tab(), first);
    press(&mut app, KeyCode::Char('1'));
    assert_eq!(app.tab(), first);
    // ctrl-d / ctrl-u are bound (half page in the content).
    let ctrl_d = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL);
    assert_eq!(app.handle_key(ctrl_d), Flow::Continue);
    // Reload, copy id, help (any key closes it).
    press(&mut app, KeyCode::Char('r'));
    assert_eq!(app.status_line(), Some("reloaded"));
    press(&mut app, KeyCode::Char('i'));
    assert!(app.status_line().is_some_and(|s| s.contains("copied id")));
    press(&mut app, KeyCode::Char('?'));
    assert!(app.help_visible());
    press(&mut app, KeyCode::Char('j'));
    assert!(!app.help_visible());
    // Edit is E; q and Q quit.
    assert!(matches!(press(&mut app, KeyCode::Char('E')), Flow::Edit(_)));
    assert_eq!(press(&mut app, KeyCode::Char('q')), Flow::Quit);
    assert_eq!(press(&mut app, KeyCode::Char('Q')), Flow::Quit);
}
