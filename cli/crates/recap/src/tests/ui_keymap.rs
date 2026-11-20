//! The standard beagle TUI keymap (`docs/CODING_STANDARDS.md` "Terminal
//! UIs"): one test pinning every binding recap shares with the other TUIs.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::tests::{fixture, key, offline};
use super::*;
use crossterm::event::KeyCode;

#[test]
fn recap_follows_the_standard_keymap() {
    let (_tmp, store) = fixture();
    let mut app = offline(store);
    // Move in the list.
    key(&mut app, KeyCode::Char('j'));
    assert_eq!(app.selected, 1);
    key(&mut app, KeyCode::Char('k'));
    assert_eq!(app.selected, 0);
    // Tab / ] / [ cycle tabs (and focus the pane); digits jump.
    key(&mut app, KeyCode::Tab);
    assert_eq!((app.tab, app.focus), (Tab::Diff, Focus::Content));
    key(&mut app, KeyCode::Char(']'));
    assert_eq!(app.tab, Tab::Comments);
    key(&mut app, KeyCode::Char(']'));
    assert_eq!(app.tab, Tab::Summary, "tabs wrap");
    key(&mut app, KeyCode::Char('['));
    assert_eq!(app.tab, Tab::Comments);
    key(&mut app, KeyCode::BackTab);
    assert_eq!(app.tab, Tab::Diff);
    key(&mut app, KeyCode::Char('1'));
    assert_eq!(app.tab, Tab::Summary);
    // Esc goes back to the list; Enter focuses the pane again.
    key(&mut app, KeyCode::Esc);
    assert_eq!(app.focus, Focus::List);
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.focus, Focus::Content);
    // GitHub writes are uppercase and ask first: D ready/draft, M merge, X close.
    for write in ['D', 'M', 'X'] {
        key(&mut app, KeyCode::Esc);
        key(&mut app, KeyCode::Char(write));
        if app.confirm.is_some() {
            key(&mut app, KeyCode::Char('n'));
        }
        // `M` first re-reads GitHub (a read); nothing may write without a yes.
        assert!(
            app.requested.iter().all(|r| r.starts_with("checking")),
            "{write} ran without a yes: {:?}",
            app.requested
        );
    }
    // Reload, help (any key closes it), q and Q quit.
    key(&mut app, KeyCode::Char('r'));
    assert!(app.status.as_deref().unwrap().starts_with("reloaded"));
    key(&mut app, KeyCode::Char('?'));
    assert!(app.help);
    key(&mut app, KeyCode::Char('j'));
    assert!(!app.help);
    key(&mut app, KeyCode::Char('Q'));
    assert!(app.should_quit());
}
