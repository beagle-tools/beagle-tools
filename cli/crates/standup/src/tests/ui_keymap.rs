//! The standard beagle TUI keymap (`docs/CODING_STANDARDS.md` "Terminal
//! UIs"): walks `beagle_tui::keymap::STANDARD` and checks standup honours
//! every key of every job it has. A new job fails to compile here until
//! standup handles it or says why it does not apply.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::tests::{fixture, key};
use super::*;
use beagle_tui::keymap::{self, Job};
use crossterm::event::KeyCode;

#[test]
fn standup_follows_the_standard_keymap() {
    for row in keymap::STANDARD {
        let (_tmp, store) = fixture();
        let mut app = App::new(store, None).unwrap();
        app.view.viewport = 10;
        app.view.layout(80);
        let keys = keymap::keys(row.job);
        match row.job {
            Job::Move => {
                app.handle_key(keys[0]);
                assert_eq!(app.selected, 1, "j");
                app.handle_key(keys[1]);
                assert_eq!(app.selected, 0, "k");
                key(&mut app, KeyCode::Enter);
                app.view.layout(80);
                app.handle_key(keys[3]);
                assert_eq!(app.view.cursor, app.view.laid.len() - 1, "G");
                app.handle_key(keys[2]);
                assert_eq!(app.view.cursor, 0, "g");
                app.handle_key(keys[4]);
                assert!(app.view.cursor > 0, "ctrl-d");
                app.handle_key(keys[5]);
                assert_eq!(app.view.cursor, 0, "ctrl-u");
            }
            Job::OpenBack => {
                app.handle_key(keys[0]);
                assert_eq!(app.focus, Focus::Content, "Enter");
                app.handle_key(keys[1]);
                assert_eq!(app.focus, Focus::List, "Esc");
            }
            Job::Tabs => {
                // n/a: standup has no tabs, and Tab must not do another job.
                app.handle_key(keys[0]);
                assert_eq!(app.focus, Focus::List, "Tab never toggles focus");
            }
            Job::HelpQuit => {
                app.handle_key(keys[0]);
                assert!(app.help, "?");
                key(&mut app, KeyCode::Char('j'));
                assert!(!app.help, "any key closes help");
                for k in &keys[1..] {
                    app.quit = false;
                    app.handle_key(*k);
                    assert!(app.quit, "{k:?}");
                }
            }
            Job::Reload => {
                app.handle_key(keys[0]);
                assert_eq!(app.status.as_deref(), Some("reloaded"));
            }
            // n/a: standup has no search and no editor (reports are
            // written by agents through `beagle standup`).
            Job::Search | Job::Edit => {}
            Job::OpenInBrowser => {
                // Pressing it would open a real browser: check it is the
                // documented key instead.
                assert!(super::keys::HELP.iter().any(|(k, _)| *k == row.keys[0]));
            }
            Job::Copy => {
                key(&mut app, KeyCode::Char('j')); // a ticket
                for k in &keys {
                    app.status = None;
                    app.handle_key(*k);
                    assert!(app.status.is_some(), "{k:?}");
                }
            }
            Job::Resize => {
                app.handle_key(keys[0]);
                assert!(app.side.is_active(), "w");
            }
        }
    }
}
