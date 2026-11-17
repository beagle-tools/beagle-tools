//! TUI tests: GitHub actions (ready, merge, close) and the archive.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::tests::{fixture, key, offline};
use super::*;
use crate::model::PrState;
use crate::store::Shelf;
use crossterm::event::KeyCode;

#[test]
fn ready_asks_first_and_cancel_does_nothing() {
    let (_tmp, store) = fixture();
    store
        .set_state(&RecapId::new("newer-2").unwrap(), PrState::Draft, None)
        .unwrap();
    let mut app = offline(store);
    assert_eq!(app.selected_row().unwrap().state, PrState::Draft);
    key(&mut app, KeyCode::Char('R'));
    assert!(matches!(
        app.confirm,
        Some(Confirm::Ready { undo: false, .. })
    ));
    key(&mut app, KeyCode::Esc);
    assert!(app.confirm.is_none());
    assert_eq!(app.requested, Vec::<String>::new(), "cancel runs nothing");
    key(&mut app, KeyCode::Char('R'));
    key(&mut app, KeyCode::Char('y'));
    assert_eq!(app.requested, ["marking ready newer#2"]);
}

#[test]
fn merge_checks_github_first_then_offers_the_allowed_methods() {
    use crate::actions::{MergeMethod, MergePlan};
    let (_tmp, store) = fixture();
    let mut app = offline(store);
    key(&mut app, KeyCode::Char('M'));
    assert_eq!(app.requested, ["checking whether newer#2 can merge"]);
    assert!(
        app.confirm.is_none(),
        "nothing to confirm until GitHub answers"
    );

    app.on_job(Job::MergeChecked {
        id: "newer-2".to_owned(),
        url: "https://github.com/o/newer/pull/2".to_owned(),
        label: "newer#2".to_owned(),
        result: Err("2 check(s) failing".to_owned()),
    });
    assert_eq!(
        app.status.as_deref(),
        Some("can't merge newer#2: 2 check(s) failing")
    );
    assert!(app.confirm.is_none());

    app.on_job(Job::MergeChecked {
        id: "newer-2".to_owned(),
        url: "https://github.com/o/newer/pull/2".to_owned(),
        label: "newer#2".to_owned(),
        result: Ok(MergePlan {
            methods: vec![MergeMethod::Squash, MergeMethod::Merge],
            auto: false,
            head: "abc".to_owned(),
        }),
    });
    assert!(matches!(
        app.confirm,
        Some(Confirm::Merge { choice: 0, .. })
    ));
    key(&mut app, KeyCode::Char('j'));
    assert!(matches!(
        app.confirm,
        Some(Confirm::Merge { choice: 1, .. })
    ));
    key(&mut app, KeyCode::Enter);
    assert_eq!(
        app.requested.last().map(String::as_str),
        Some("merging newer#2 (merge commit)")
    );
}

#[test]
fn guards_explain_why_an_action_isnt_offered() {
    let (_tmp, store) = fixture();
    store
        .set_state(&RecapId::new("newer-2").unwrap(), PrState::Draft, None)
        .unwrap();
    store
        .set_state(&RecapId::new("older-1").unwrap(), PrState::Merged, None)
        .unwrap();
    let mut app = offline(store);
    key(&mut app, KeyCode::Char('M'));
    assert!(app
        .status
        .as_deref()
        .unwrap()
        .contains("R marks it ready first"));
    key(&mut app, KeyCode::Char('j')); // older-1, merged
    key(&mut app, KeyCode::Char('X'));
    assert!(app.status.as_deref().unwrap().contains("already merged"));
    key(&mut app, KeyCode::Char('R'));
    assert!(app
        .status
        .as_deref()
        .unwrap()
        .contains("nothing to mark ready"));
    assert_eq!(app.requested, Vec::<String>::new());
}

#[test]
fn closing_archives_and_the_archive_view_restores() {
    let (_tmp, store) = fixture();
    let mut app = offline(store.clone());
    key(&mut app, KeyCode::Char('X'));
    assert!(matches!(app.confirm, Some(Confirm::Close { .. })));
    key(&mut app, KeyCode::Char('y'));
    assert_eq!(app.requested, ["closing newer#2"]);
    // GitHub says it closed:
    app.on_job(Job::Done {
        id: "newer-2".to_owned(),
        label: "newer#2".to_owned(),
        closed: true,
        result: Ok("closed".to_owned()),
    });
    assert!(app
        .status
        .as_deref()
        .unwrap()
        .contains("closed and archived"));
    assert_eq!(
        app.rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
        ["older-1"]
    );

    key(&mut app, KeyCode::Char('A'));
    assert_eq!(app.shelf, Shelf::Archived);
    assert_eq!(
        app.rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
        ["newer-2"]
    );
    assert!(app.recap.is_some(), "archived recaps still open and read");
    key(&mut app, KeyCode::Char('M'));
    assert!(app
        .status
        .as_deref()
        .unwrap()
        .contains("this is the archive"));
    key(&mut app, KeyCode::Char('U'));
    assert_eq!(app.rows.len(), 0);
    key(&mut app, KeyCode::Char('A'));
    assert_eq!(app.rows.len(), 2, "restored");

    // `a` archives locally without touching GitHub.
    key(&mut app, KeyCode::Char('a'));
    assert_eq!(app.rows.len(), 1);
    assert_eq!(
        app.requested,
        ["closing newer#2"],
        "no GitHub call for a local archive"
    );
}

#[test]
fn a_refused_action_says_so_and_changes_nothing() {
    let (_tmp, store) = fixture();
    let mut app = offline(store);
    app.on_job(Job::Done {
        id: "newer-2".to_owned(),
        label: "newer#2".to_owned(),
        closed: true,
        result: Err("pull request is already closed".to_owned()),
    });
    assert_eq!(
        app.status.as_deref(),
        Some("newer#2: gh refused: pull request is already closed")
    );
    assert_eq!(app.rows.len(), 2, "not archived when GitHub refused");
}

#[test]
fn confirm_dialog_renders() {
    use crate::actions::{MergeMethod, MergePlan};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    let (_tmp, store) = fixture();
    let mut app = offline(store);
    app.confirm = Some(Confirm::Merge {
        id: "newer-2".to_owned(),
        url: String::new(),
        label: "newer#2".to_owned(),
        plan: MergePlan {
            methods: vec![MergeMethod::Squash],
            auto: true,
            head: "abc".to_owned(),
        },
        choice: 0,
    });
    let mut t = Terminal::new(TestBackend::new(120, 30)).unwrap();
    t.draw(|f| view::draw(f, &mut app)).unwrap();
    let screen: String = t
        .backend()
        .buffer()
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect();
    assert!(screen.contains("enable auto-merge?"));
    assert!(screen.contains("squash"));
    assert!(screen.contains("(repo default)"));
    assert!(screen.contains("exact commit"));
}
