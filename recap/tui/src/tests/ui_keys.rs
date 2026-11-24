//! TUI tests for `ui/keys.rs` paths the keymap and action tests don't
//! reach: the focus walk into the agent review and back, scrolling that
//! review, ctrl-c, the tab digits and picking another PR.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::tests::{fixture, key, offline};
use super::*;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn ctrl(app: &mut App, c: char) {
    app.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL));
}

#[test]
fn enter_walks_in_to_the_agent_review_and_esc_walks_back_out() {
    let (_tmp, store) = fixture();
    let mut app = offline(store);
    key(&mut app, KeyCode::Char('3'));
    assert_eq!((app.tab, app.focus), (Tab::Comments, Focus::Content));

    key(&mut app, KeyCode::Enter);
    assert_eq!(app.focus, Focus::Review);
    assert!(app.status.as_deref().unwrap().contains("agent review"));
    // Enter again stays put: the review is the innermost pane.
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.focus, Focus::Review);

    key(&mut app, KeyCode::Esc);
    assert_eq!(app.focus, Focus::Content, "esc leaves the review first");
    key(&mut app, KeyCode::Esc);
    assert_eq!(app.focus, Focus::List);
}

#[test]
fn enter_on_summary_or_diff_stops_at_the_pane() {
    let (_tmp, store) = fixture();
    let mut app = offline(store);
    for tab_key in ['1', '2'] {
        key(&mut app, KeyCode::Esc);
        key(&mut app, KeyCode::Char(tab_key));
        key(&mut app, KeyCode::Enter);
        assert_eq!(app.focus, Focus::Content, "tab {tab_key}");
    }
}

#[test]
fn movement_keys_scroll_the_review_while_it_has_focus() {
    let (_tmp, store) = fixture();
    let mut app = offline(store);
    key(&mut app, KeyCode::Char('3'));
    key(&mut app, KeyCode::Enter);
    let selected = app.selected;

    key(&mut app, KeyCode::Char('j'));
    key(&mut app, KeyCode::Down);
    assert_eq!(app.review_scroll, 2);
    key(&mut app, KeyCode::Char('k'));
    assert_eq!(app.review_scroll, 1);
    key(&mut app, KeyCode::Char('G'));
    assert_eq!(app.review_scroll, u16::MAX);
    key(&mut app, KeyCode::Char('g'));
    assert_eq!(app.review_scroll, 0);
    assert_eq!(app.selected, selected, "the PR list never moved");
    assert_eq!(app.focus, Focus::Review);
}

#[test]
fn capital_j_and_k_scroll_the_review_from_the_thread() {
    let (_tmp, store) = fixture();
    let mut app = offline(store);
    key(&mut app, KeyCode::Char('3'));
    key(&mut app, KeyCode::Char('J'));
    assert_eq!(app.review_scroll, 3);
    key(&mut app, KeyCode::Char('K'));
    assert_eq!(app.review_scroll, 0);
    assert_eq!(app.focus, Focus::Content, "the thread keeps focus");
}

#[test]
fn ctrl_c_quits_from_anywhere() {
    for setup in [None, Some('?'), Some('w')] {
        let (_tmp, store) = fixture();
        let mut app = offline(store);
        if let Some(c) = setup {
            key(&mut app, KeyCode::Char(c));
        }
        ctrl(&mut app, 'c');
        assert!(app.should_quit(), "after {setup:?}");
    }
}

#[test]
fn tab_digits_switch_and_focus_the_pane() {
    let (_tmp, store) = fixture();
    let mut app = offline(store);
    for (c, tab) in [('2', Tab::Diff), ('3', Tab::Comments), ('1', Tab::Summary)] {
        key(&mut app, KeyCode::Esc);
        key(&mut app, KeyCode::Char(c));
        assert_eq!((app.tab, app.focus), (tab, Focus::Content), "{c}");
    }
}

#[test]
fn picking_another_pr_opens_it_on_its_summary() {
    let (_tmp, store) = fixture();
    let mut app = offline(store);
    key(&mut app, KeyCode::Char('2'));
    key(&mut app, KeyCode::Esc);
    assert_eq!((app.tab, app.focus), (Tab::Diff, Focus::List));
    key(&mut app, KeyCode::Char('j'));
    assert_eq!(app.selected, 1);
    assert_eq!(app.tab, Tab::Summary);
    // Moving past the end keeps the last PR (and its tab).
    key(&mut app, KeyCode::Char('2'));
    key(&mut app, KeyCode::Esc);
    key(&mut app, KeyCode::Char('j'));
    assert_eq!((app.selected, app.tab), (1, Tab::Diff));
}

#[test]
fn n_on_comments_without_threads_says_so_and_stays_put() {
    let (_tmp, store) = fixture();
    let mut app = offline(store);
    key(&mut app, KeyCode::Char('3'));
    for c in ['n', 'N'] {
        key(&mut app, KeyCode::Char(c));
        assert_eq!(app.tab, Tab::Comments, "{c} never leaves Comments");
        assert_eq!(
            app.status.as_deref(),
            Some("no comment threads on this PR"),
            "{c}"
        );
    }
}
