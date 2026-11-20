//! The standard beagle TUI keymap (`docs/CODING_STANDARDS.md` "Terminal
//! UIs"): walks `beagle_tui::keymap::STANDARD` and checks rca honours
//! every key of every job it has. A new job fails to compile here until
//! rca handles it or says why it does not apply.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use beagle_tui::keymap::{self, Job};
use crossterm::event::KeyEvent;

use crate::ui::testutil::app_with;

use super::*;

fn press(app: &mut App, key: KeyEvent) -> Flow {
    app.handle_key(key)
}

#[test]
fn rca_follows_the_standard_keymap() {
    for row in keymap::STANDARD {
        let mut app = app_with(3);
        let keys = keymap::keys(row.job);
        match row.job {
            Job::Move => {
                // j, k, g, G in the list; ctrl-d / ctrl-u in the content.
                press(&mut app, keys[0]);
                assert_eq!(app.selected, 1, "j");
                press(&mut app, keys[1]);
                assert_eq!(app.selected, 0, "k");
                press(&mut app, keys[3]);
                assert_eq!(app.selected, 2, "G");
                press(&mut app, keys[2]);
                assert_eq!(app.selected, 0, "g");
                press(&mut app, KeyEvent::from(KeyCode::Enter));
                for k in &keys[4..] {
                    assert_eq!(press(&mut app, *k), Flow::Continue);
                    assert_eq!(app.focus(), Focus::Content);
                }
            }
            Job::OpenBack => {
                press(&mut app, keys[0]);
                assert_eq!(app.focus(), Focus::Content, "Enter opens");
                press(&mut app, keys[1]);
                assert_eq!(app.focus(), Focus::List, "Esc goes back");
            }
            Job::Tabs => {
                let first = app.tab();
                press(&mut app, keys[0]);
                assert_ne!(app.tab(), first, "Tab");
                press(&mut app, keys[2]);
                assert_eq!(app.tab(), first, "[");
                press(&mut app, keys[1]);
                assert_ne!(app.tab(), first, "]");
            }
            Job::HelpQuit => {
                press(&mut app, keys[0]);
                assert!(app.help_visible(), "?");
                press(&mut app, KeyEvent::from(KeyCode::Char('j')));
                assert!(!app.help_visible(), "any key closes help");
                for k in &keys[1..] {
                    assert_eq!(press(&mut app, *k), Flow::Quit);
                }
            }
            Job::Reload => {
                press(&mut app, keys[0]);
                assert_eq!(app.status_line(), Some("reloaded"));
            }
            Job::Search => {
                press(&mut app, keys[0]);
                assert!(app.content_search().is_some(), "/ searches");
            }
            Job::OpenInBrowser => {
                // `o` opens the links picker (attached PRs / URLs) rather
                // than one browser tab; it must not be anything else.
                assert_eq!(press(&mut app, keys[0]), Flow::Continue);
            }
            Job::Edit => {
                assert!(matches!(press(&mut app, keys[0]), Flow::Edit(_)));
            }
            Job::Copy => {
                // y yanks a content line, Y has no URL to copy in rca, i
                // copies the incident id.
                press(&mut app, KeyEvent::from(KeyCode::Enter));
                press(&mut app, keys[0]);
                assert!(app.status_line().is_some(), "y");
                press(&mut app, keys[2]);
                assert!(app.status_line().is_some_and(|s| s.contains("copied id")));
            }
            Job::Resize => {
                press(&mut app, keys[0]);
                assert!(app.side.is_active(), "w");
            }
        }
    }
}
