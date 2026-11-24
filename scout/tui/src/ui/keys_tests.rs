//! Tests for `ui::keys`.
// Tests fail loudly on setup errors; unwrap/expect/panic is the clearest way.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;
use crate::model::{Slug, TaskStatus};
use crate::store::Store;

fn press(app: &mut App, code: KeyCode) -> Flow {
    app.handle_key(KeyEvent::new(code, KeyModifiers::NONE))
}

fn app_with_project() -> (tempfile::TempDir, App) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path());
    store
        .create_project(&Slug::parse("p").unwrap(), "P", vec![])
        .unwrap();
    for slug in ["a", "b", "c"] {
        store
            .add_task("p", &Slug::parse(slug).unwrap(), slug, vec![])
            .unwrap();
    }
    store.set_task_status("p", "a", TaskStatus::Done).unwrap();
    store
        .set_task_status("p", "b", TaskStatus::Working)
        .unwrap();
    let app = App::new(store).unwrap();
    (dir, app)
}

#[test]
fn opening_a_project_lands_on_the_working_task() {
    let (_dir, mut app) = app_with_project();
    assert_eq!(press(&mut app, KeyCode::Enter), Flow::Continue);
    assert_eq!(app.screen, Screen::Tasks);
    assert_eq!(app.selected_task().unwrap().id, "02-b");
    assert_eq!(app.tab, Tab::Task);
}

#[test]
fn back_peels_one_layer_at_a_time() {
    let (_dir, mut app) = app_with_project();
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.focus, Focus::Content);
    press(&mut app, KeyCode::Char('b'));
    assert_eq!((app.screen, app.focus), (Screen::Tasks, Focus::List));
    press(&mut app, KeyCode::Esc);
    assert_eq!(app.screen, Screen::Projects);
}

#[test]
fn tabs_cycle_and_jump() {
    let (_dir, mut app) = app_with_project();
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Tab);
    assert_eq!(app.tab, Tab::Outcome);
    app.ensure_pane();
    assert!(
        app.current_file().unwrap().ends_with("02-b.md"),
        "the Outcome tab shows the task's own file"
    );
    press(&mut app, KeyCode::Tab);
    assert_eq!(app.tab, Tab::Session);
    press(&mut app, KeyCode::Tab);
    assert_eq!(app.tab, Tab::Project);
    press(&mut app, KeyCode::Char('5'));
    assert_eq!(app.tab, Tab::Log);
    press(&mut app, KeyCode::Tab);
    assert_eq!(app.tab, Tab::Task);
    press(&mut app, KeyCode::BackTab);
    assert_eq!(app.tab, Tab::Log);
    app.ensure_pane();
    assert!(app.current_file().unwrap().ends_with("log.md"));
}

#[test]
fn selection_clamps() {
    let (_dir, mut app) = app_with_project();
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Char('G'));
    assert_eq!(app.selected_task().unwrap().id, "03-c");
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(app.selected_task().unwrap().id, "03-c");
    press(&mut app, KeyCode::Char('g'));
    press(&mut app, KeyCode::Char('k'));
    assert_eq!(app.selected_task().unwrap().id, "01-a");
}

#[test]
fn follow_pins_and_scrolling_up_releases() {
    let (_dir, mut app) = app_with_project();
    app.max_scroll = 10;
    press(&mut app, KeyCode::Char('F'));
    assert!(app.follow);
    assert_eq!(app.scroll, usize::MAX);
    app.handle_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
    assert!(!app.follow);
    assert!(app.scroll < 10);
}

#[test]
fn help_swallows_the_next_key() {
    let (_dir, mut app) = app_with_project();
    press(&mut app, KeyCode::Char('?'));
    assert!(app.help);
    press(&mut app, KeyCode::Enter);
    assert!(!app.help);
    assert_eq!(app.screen, Screen::Projects);
}

#[test]
fn quit_and_edit() {
    let (_dir, mut app) = app_with_project();
    match press(&mut app, KeyCode::Char('E')) {
        Flow::Edit(path) => assert!(path.ends_with("project.md")),
        other => panic!("expected edit, got {other:?}"),
    }
    assert_eq!(press(&mut app, KeyCode::Char('Q')), Flow::Quit);
    assert_eq!(press(&mut app, KeyCode::Char('q')), Flow::Quit);
    let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert_eq!(app.handle_key(ctrl_c), Flow::Quit);
}

/// Walks `beagle_tui::keymap::STANDARD` (`docs/CODING_STANDARDS.md`):
/// a new job fails to compile here until scout handles it or says why
/// it does not apply.
#[test]
fn scout_follows_the_standard_keymap() {
    use beagle_tui::keymap::{self, Job};
    for row in keymap::STANDARD {
        let (_dir, mut app) = app_with_project();
        let keys = keymap::keys(row.job);
        press(&mut app, KeyCode::Enter); // into the project's tasks
        match row.job {
            Job::Move => {
                for (k, want) in keys[..4].iter().zip(["03-c", "02-b", "01-a", "03-c"]) {
                    app.handle_key(*k);
                    assert_eq!(app.selected_task().unwrap().id, want, "{k:?}");
                }
                app.max_scroll = 10;
                app.viewport_height = 4;
                app.handle_key(keys[4]);
                assert_eq!(app.scroll, 2, "ctrl-d");
                app.handle_key(keys[5]);
                assert_eq!(app.scroll, 0, "ctrl-u");
            }
            Job::OpenBack => {
                app.handle_key(keys[0]);
                assert_eq!(app.focus, Focus::Content, "Enter");
                app.handle_key(keys[1]);
                assert_eq!(app.focus, Focus::List, "Esc");
                app.handle_key(keys[0]);
                app.handle_key(keys[2]);
                assert_eq!(app.focus, Focus::List, "b");
            }
            Job::Tabs => {
                for (k, want) in keys.iter().zip([Tab::Outcome, Tab::Session, Tab::Outcome]) {
                    app.handle_key(*k);
                    assert_eq!(app.tab, want, "{k:?}");
                }
            }
            Job::HelpQuit => {
                app.handle_key(keys[0]);
                assert!(app.help, "?");
                press(&mut app, KeyCode::Char('j'));
                assert!(!app.help, "any key closes help");
                for k in &keys[1..] {
                    assert_eq!(app.handle_key(*k), Flow::Quit);
                }
            }
            Job::Reload => {
                app.handle_key(keys[0]);
                assert!(app.status.is_some(), "r");
            }
            // n/a: scout has no search, nothing to open in a browser and
            // nothing to copy (yet).
            Job::Search | Job::OpenInBrowser | Job::Copy => {}
            Job::Edit => assert!(matches!(app.handle_key(keys[0]), Flow::Edit(_))),
            Job::Resize => {
                app.handle_key(keys[0]);
                assert!(app.side.is_active(), "w");
            }
        }
    }
}

#[test]
fn reload_announces_status_changes_and_keeps_selection() {
    let (_dir, mut app) = app_with_project();
    press(&mut app, KeyCode::Enter);
    app.store
        .set_task_status("p", "c", TaskStatus::Working)
        .unwrap();
    let note = app.reload().unwrap();
    assert!(note.contains("03-c → working"), "{note}");
    assert_eq!(app.selected_task().unwrap().id, "02-b");
}

#[test]
fn empty_store_is_harmless() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(Store::new(dir.path())).unwrap();
    for code in [
        KeyCode::Enter,
        KeyCode::Char('j'),
        KeyCode::Char('E'),
        KeyCode::Tab,
    ] {
        assert_eq!(press(&mut app, code), Flow::Continue);
    }
    assert_eq!(app.screen, Screen::Projects);
}
