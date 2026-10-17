//! Key-handling tests for the editing overlays: status picker, tags
//! editor, and the PR/ticket attach prompt (`ui::keys`).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use crate::ui::testutil::{app_with, press};

use super::*;

#[test]
fn t_opens_the_status_picker_and_enter_applies_the_stage() {
    use crate::model::Status;

    let mut app = app_with(1);
    assert_eq!(
        app.selected_rca().expect("selected").meta.status,
        Status::Investigating
    );

    press(&mut app, KeyCode::Char('t'));
    let picker = app.status_picker.as_ref().expect("picker open");
    assert_eq!(picker.current, Status::Investigating);
    assert_eq!(picker.selected, 0, "current stage highlighted");

    // Move to `review` and apply; the manifest write reloads the list.
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Enter);
    assert!(app.status_picker.is_none());
    assert_eq!(
        app.selected_rca().expect("selected").meta.status,
        Status::Review
    );
    assert!(
        app.status_line().is_some_and(|s| s.contains("review")),
        "status announces the change"
    );
}

#[test]
fn status_picker_esc_cancels_and_repicking_current_writes_nothing() {
    use crate::model::Status;

    let mut app = app_with(1);
    let before = app
        .store
        .read_meta(&app.selected_rca().expect("selected").id.clone())
        .expect("meta");

    // Esc: no change.
    press(&mut app, KeyCode::Char('t'));
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Esc);
    assert!(app.status_picker.is_none());
    assert_eq!(
        app.selected_rca().expect("selected").meta.status,
        Status::Investigating
    );

    // Enter on the current stage: closes without stamping `updated`.
    press(&mut app, KeyCode::Char('t'));
    press(&mut app, KeyCode::Enter);
    assert!(app.status_picker.is_none());
    let after = app
        .store
        .read_meta(&app.selected_rca().expect("selected").id.clone())
        .expect("meta");
    assert_eq!(before.updated, after.updated, "no spurious write");

    // Nothing selected: t says so.
    let mut empty = app_with(0);
    press(&mut empty, KeyCode::Char('t'));
    assert!(empty.status_picker.is_none());
    assert!(empty
        .status_line()
        .is_some_and(|s| s.contains("nothing to set")));
}

#[test]
fn hash_opens_the_tags_editor_and_edits_write_through() {
    let mut app = app_with(1);
    let id = app.selected_rca().expect("selected").id.clone();

    press(&mut app, KeyCode::Char('#'));
    assert!(app.tags_editor.is_some(), "editor open");

    // Type a new tag: `a` enters typing mode, chars build it, enter lands
    // it on disk immediately.
    press(&mut app, KeyCode::Char('a'));
    for c in "skip-final-review".chars() {
        press(&mut app, KeyCode::Char(c));
    }
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        app.store.read_meta(&id).expect("meta").tags,
        vec!["skip-final-review".to_owned()],
        "written to the manifest"
    );
    let editor = app.tags_editor.as_ref().expect("still open");
    assert_eq!(editor.tags, vec!["skip-final-review".to_owned()]);
    assert!(editor.typing.is_none(), "back in navigation mode");

    // `d` on the tag deletes it from disk too.
    press(&mut app, KeyCode::Char('k'));
    press(&mut app, KeyCode::Char('d'));
    assert_eq!(
        app.store.read_meta(&id).expect("meta").tags,
        [] as [String; 0]
    );

    // Esc closes; nothing selected on an empty store just reports.
    press(&mut app, KeyCode::Esc);
    assert!(app.tags_editor.is_none());
    let mut empty = app_with(0);
    press(&mut empty, KeyCode::Char('#'));
    assert!(empty.tags_editor.is_none());
}

#[test]
fn tags_editor_rejects_whitespace_and_esc_peels_typing_first() {
    let mut app = app_with(1);
    let id = app.selected_rca().expect("selected").id.clone();

    press(&mut app, KeyCode::Char('#'));
    press(&mut app, KeyCode::Char('a'));
    for c in "two words".chars() {
        press(&mut app, KeyCode::Char(c));
    }
    press(&mut app, KeyCode::Enter);
    assert!(
        app.store.read_meta(&id).expect("meta").tags.is_empty(),
        "whitespace tag never lands"
    );
    assert!(app.status_line().is_some_and(|s| s.contains("kebab-case")));

    // Esc from typing returns to navigation, second esc closes.
    press(&mut app, KeyCode::Char('a'));
    press(&mut app, KeyCode::Esc);
    assert!(app.tags_editor.as_ref().expect("open").typing.is_none());
    press(&mut app, KeyCode::Esc);
    assert!(app.tags_editor.is_none());
}

#[test]
fn shift_p_attaches_a_pr_via_the_prompt() {
    let mut app = app_with(1);
    let id = app.selected_rca().expect("selected").id.clone();

    // P opens the prompt; typing builds the URL.
    press(&mut app, KeyCode::Char('P'));
    assert!(app.attach_prompt().is_some(), "prompt open");
    for c in "https://github.com/o/r/pull/7".chars() {
        press(&mut app, KeyCode::Char(c));
    }
    press(&mut app, KeyCode::Enter);

    assert!(app.attach_prompt().is_none(), "prompt closes on attach");
    assert_eq!(
        app.store.read_meta(&id).expect("meta").prs,
        vec!["https://github.com/o/r/pull/7".to_owned()],
        "PR written to the manifest"
    );
    assert!(app.status_line().is_some_and(|s| s.contains("attached")));

    // A bad URL keeps the prompt open with an error.
    press(&mut app, KeyCode::Char('P'));
    for c in "not-a-url".chars() {
        press(&mut app, KeyCode::Char(c));
    }
    press(&mut app, KeyCode::Enter);
    assert!(
        app.attach_prompt().is_some(),
        "invalid URL keeps the prompt open"
    );
    press(&mut app, KeyCode::Esc);
    assert!(app.attach_prompt().is_none(), "esc cancels");
}

#[test]
fn shift_i_attaches_a_ticket_via_the_prompt() {
    let mut app = app_with(1);
    let id = app.selected_rca().expect("selected").id.clone();

    press(&mut app, KeyCode::Char('I'));
    assert!(app.attach_prompt().is_some(), "ticket prompt open");
    for c in "https://linear.app/acme/issue/ENG-42/title".chars() {
        press(&mut app, KeyCode::Char(c));
    }
    press(&mut app, KeyCode::Enter);
    assert!(app.attach_prompt().is_none(), "closes on attach");
    assert_eq!(
        app.store.read_meta(&id).expect("meta").tickets,
        vec!["https://linear.app/acme/issue/ENG-42/title".to_owned()]
    );

    // A non-ticket reference keeps the prompt open (invalid).
    press(&mut app, KeyCode::Char('I'));
    for c in "just-text".chars() {
        press(&mut app, KeyCode::Char(c));
    }
    press(&mut app, KeyCode::Enter);
    assert!(app.attach_prompt().is_some(), "invalid ref stays open");
    press(&mut app, KeyCode::Esc);
}
