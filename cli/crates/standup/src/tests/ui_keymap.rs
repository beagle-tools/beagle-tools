//! The standard beagle TUI keymap (`docs/CODING_STANDARDS.md` "Terminal
//! UIs"): one test pinning every binding standup shares with the other TUIs.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::tests::{fixture, key};
use super::*;
use crossterm::event::KeyCode;

#[test]
fn standup_follows_the_standard_keymap() {
    let (_tmp, store) = fixture();
    let mut app = App::new(store, None).unwrap();
    app.view.viewport = 10;
    app.view.layout(80);
    // Move in the list.
    key(&mut app, KeyCode::Char('j'));
    assert_eq!(app.selected, 1);
    key(&mut app, KeyCode::Char('k'));
    assert_eq!(app.selected, 0);
    // Tab never toggles focus (it cycles tabs, and standup has none).
    key(&mut app, KeyCode::Tab);
    assert_eq!(app.focus, Focus::List);
    // Enter opens the write-up; g/G move in it; Esc goes back.
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.focus, Focus::Content);
    app.view.layout(80); // the draw loop lays the rebuilt doc out
    key(&mut app, KeyCode::Char('G'));
    assert_eq!(app.view.cursor, app.view.laid.len() - 1);
    key(&mut app, KeyCode::Char('g'));
    assert_eq!(app.view.cursor, 0);
    key(&mut app, KeyCode::Esc);
    assert_eq!(app.focus, Focus::List);
    // Reload, help (any key closes it), q and Q quit.
    key(&mut app, KeyCode::Char('r'));
    assert_eq!(app.status.as_deref(), Some("reloaded"));
    key(&mut app, KeyCode::Char('?'));
    assert!(app.help);
    key(&mut app, KeyCode::Char('j'));
    assert!(!app.help);
    key(&mut app, KeyCode::Char('Q'));
    assert!(app.quit);
}
