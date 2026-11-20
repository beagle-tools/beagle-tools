//! The standard beagle TUI keymap (`docs/CODING_STANDARDS.md` "Terminal
//! UIs"): walks `beagle_tui::keymap::STANDARD` and checks recap honours
//! every key of every job it has. A new job fails to compile here until
//! recap handles it or says why it does not apply.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::tests::{fixture, key, offline};
use super::*;
use beagle_tui::keymap::{self, Job, GITHUB_WRITES};
use crossterm::event::KeyCode;

#[test]
fn recap_follows_the_standard_keymap() {
    for row in keymap::STANDARD {
        let (_tmp, store) = fixture();
        let mut app = offline(store);
        let keys = keymap::keys(row.job);
        match row.job {
            Job::Move => {
                app.handle_key(keys[0]);
                assert_eq!(app.selected, 1, "j");
                app.handle_key(keys[1]);
                assert_eq!(app.selected, 0, "k");
                key(&mut app, KeyCode::Enter);
                for k in &keys[2..] {
                    app.handle_key(*k);
                    assert_eq!(app.focus, Focus::Content, "{k:?} keeps the pane");
                }
            }
            Job::OpenBack => {
                app.handle_key(keys[0]);
                assert_eq!(app.focus, Focus::Content, "Enter");
                app.handle_key(keys[1]);
                assert_eq!(app.focus, Focus::List, "Esc");
            }
            Job::Tabs => {
                for (k, want) in keys.iter().zip([Tab::Diff, Tab::Comments, Tab::Diff]) {
                    app.handle_key(*k);
                    assert_eq!(app.tab, want, "{k:?}");
                }
            }
            Job::HelpQuit => {
                app.handle_key(keys[0]);
                assert!(app.help, "?");
                key(&mut app, KeyCode::Char('j'));
                assert!(!app.help, "any key closes help");
                for k in &keys[1..] {
                    app.quit = false;
                    app.handle_key(*k);
                    assert!(app.should_quit(), "{k:?}");
                }
            }
            Job::Reload => {
                app.handle_key(keys[0]);
                assert!(app.status.as_deref().unwrap().starts_with("reloaded"));
            }
            // n/a: recap has no search and no editor (recaps are written by
            // agents through `beagle recap`).
            Job::Search | Job::Edit => {}
            Job::OpenInBrowser => {
                // Pressing it would open a real browser: check it is the
                // documented key instead.
                assert!(super::keys::HELP.iter().any(|(k, _)| *k == row.keys[0]));
            }
            Job::Copy => {
                // y / Y / i copy line, URL, id: each reports in the status
                // line rather than doing nothing.
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

#[test]
fn recap_github_writes_ask_first() {
    let (_tmp, store) = fixture();
    let mut app = offline(store);
    for (write, _) in GITHUB_WRITES {
        let Some(c) = write.chars().next() else {
            continue;
        };
        key(&mut app, KeyCode::Char(c));
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
}
