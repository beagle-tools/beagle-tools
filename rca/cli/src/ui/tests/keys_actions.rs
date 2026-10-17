//! Key-handling tests for one-shot actions: edit, delete, the errors
//! overlay, yank, the help sheet, and optional-tab visibility (`ui::keys`).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use crate::ui::testutil::{app_with, press};

use super::*;

#[test]
fn shift_e_requests_an_edit_of_the_current_tabs_file() {
    let mut app = app_with(1);
    match press(&mut app, KeyCode::Char('E')) {
        Flow::Edit(path) => assert!(
            path.ends_with("rca-0/summary.md"),
            "summary tab edits summary.md: {path:?}"
        ),
        other => panic!("expected Flow::Edit, got {other:?}"),
    }

    press(&mut app, KeyCode::Char('9')); // Log tab
    match press(&mut app, KeyCode::Char('E')) {
        Flow::Edit(path) => assert!(path.ends_with("rca-0/log.md")),
        other => panic!("expected Flow::Edit, got {other:?}"),
    }
}

#[test]
fn shift_e_on_an_empty_diagram_tab_or_empty_store_stays_put() {
    let mut app = app_with(1);
    press(&mut app, KeyCode::Char('7')); // Diagrams tab, no diagram files
    assert_eq!(
        press(&mut app, KeyCode::Char('E')),
        Flow::Continue,
        "nothing to edit without diagrams"
    );
    assert!(
        app.status_line().is_some_and(|s| s.contains("diagram")),
        "status explains why"
    );

    let mut empty = app_with(0);
    assert_eq!(press(&mut empty, KeyCode::Char('E')), Flow::Continue);
}

#[test]
fn shift_e_edits_the_current_diagram_when_one_exists() {
    let mut app = app_with(1);
    let id = crate::model::RcaId::new("rca-0").expect("id");
    let dir = app.store.workspace_dir(&id).join("diagrams");
    std::fs::write(dir.join("01-topology.txt"), "box").expect("write");
    std::fs::write(dir.join("02-fix.txt"), "box2").expect("write");
    press(&mut app, KeyCode::Char('7'));
    match press(&mut app, KeyCode::Char('E')) {
        Flow::Edit(path) => assert!(path.ends_with("diagrams/01-topology.txt")),
        other => panic!("expected Flow::Edit, got {other:?}"),
    }
}

#[test]
fn shift_d_confirms_before_deleting_and_only_y_deletes() {
    let mut app = app_with(2);
    let doomed = app.selected_rca().expect("selected").id.clone();
    let dir = app.store.workspace_dir(&doomed);

    // D opens the confirmation; the list is untouched until an answer.
    press(&mut app, KeyCode::Char('D'));
    assert!(app.confirm_delete.is_some(), "popup open");
    assert_eq!(app.rcas.len(), 2);

    // Enter must NOT confirm — a queued enter from navigation would
    // otherwise delete without the user ever seeing the popup.
    press(&mut app, KeyCode::Enter);
    assert!(app.confirm_delete.is_some(), "enter is ignored");
    assert!(dir.exists());

    // n cancels, everything intact.
    press(&mut app, KeyCode::Char('n'));
    assert!(app.confirm_delete.is_none());
    assert!(dir.exists());
    assert_eq!(app.rcas.len(), 2);

    // D then y deletes and reloads.
    press(&mut app, KeyCode::Char('D'));
    press(&mut app, KeyCode::Char('y'));
    assert!(app.confirm_delete.is_none());
    assert!(!dir.exists(), "workspace directory removed");
    assert_eq!(app.rcas.len(), 1);
    assert!(
        app.status_line().is_some_and(|s| s.contains("deleted")),
        "status announces the delete"
    );
}

#[test]
fn shift_d_is_list_only_and_esc_cancels() {
    let mut app = app_with(1);

    // Content focus: D is not a delete trigger while reading.
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Char('D'));
    assert!(app.confirm_delete.is_none(), "no popup from content focus");

    // Back on the list, esc closes the popup without deleting.
    press(&mut app, KeyCode::Esc);
    press(&mut app, KeyCode::Char('D'));
    assert!(app.confirm_delete.is_some());
    press(&mut app, KeyCode::Esc);
    assert!(app.confirm_delete.is_none());
    assert_eq!(app.rcas.len(), 1);

    // Nothing selected: D says so instead of opening an empty popup.
    let mut empty = app_with(0);
    press(&mut empty, KeyCode::Char('D'));
    assert!(empty.confirm_delete.is_none());
    assert!(empty
        .status_line()
        .is_some_and(|s| s.contains("nothing to delete")));
}

#[test]
fn bang_opens_the_errors_overlay_only_when_there_are_problems() {
    let mut app = app_with(1);

    // Clean store: `!` reports, opens nothing.
    press(&mut app, KeyCode::Char('!'));
    assert!(!app.errors_visible());
    assert!(app
        .status_line()
        .is_some_and(|s| s.contains("no load errors")));

    // Plant a manifest-less directory under rcas/ — a broken workspace.
    std::fs::create_dir_all(app.store.root().join("rcas").join("husk")).expect("mkdir");
    app.reload();
    assert_eq!(app.broken().len(), 1, "surfaced as broken");

    // `!` now opens the overlay; j scrolls; esc closes.
    press(&mut app, KeyCode::Char('!'));
    assert!(app.errors_visible());

    // `c` copies the errors without closing the overlay.
    press(&mut app, KeyCode::Char('c'));
    assert!(app.errors_visible(), "copy leaves the overlay open");
    assert!(
        app.status_line()
            .is_some_and(|s| s.contains("copied load errors")),
        "copy is confirmed in the status bar"
    );

    press(&mut app, KeyCode::Esc);
    assert!(!app.errors_visible());

    // While it's open every other key is intercepted (no quit leak).
    press(&mut app, KeyCode::Char('!'));
    assert_eq!(press(&mut app, KeyCode::Char('j')), Flow::Continue);
    assert!(app.errors_visible());
    press(&mut app, KeyCode::Char('!'));
    assert!(!app.errors_visible());
}

#[test]
fn shift_y_yanks_the_selected_incident_id() {
    let mut app = app_with(1);
    let id = app.selected_rca().expect("selected").id.to_string();
    press(&mut app, KeyCode::Char('Y'));
    assert!(
        app.status_line()
            .is_some_and(|s| s.contains("copied id") && s.contains(&id)),
        "status confirms the id copy: {:?}",
        app.status_line()
    );

    // Nothing selected: reports instead of copying.
    let mut empty = app_with(0);
    press(&mut empty, KeyCode::Char('Y'));
    assert!(empty
        .status_line()
        .is_some_and(|s| s.contains("no incident selected")));
}

#[test]
fn help_pane_f_filters_and_esc_peels_then_closes() {
    let mut app = app_with(1);

    // `?` opens help, no filter yet.
    press(&mut app, KeyCode::Char('?'));
    assert!(app.help_visible());
    assert_eq!(app.help_filter(), None);

    // `f` starts filtering; typing builds the query (not a close).
    press(&mut app, KeyCode::Char('f'));
    assert_eq!(app.help_filter(), Some(""));
    for c in "tag".chars() {
        press(&mut app, KeyCode::Char(c));
    }
    assert_eq!(app.help_filter(), Some("tag"));
    assert!(app.help_visible(), "typing does not close help");
    press(&mut app, KeyCode::Backspace);
    assert_eq!(app.help_filter(), Some("ta"));

    // esc peels the filter first, then closes.
    press(&mut app, KeyCode::Esc);
    assert_eq!(app.help_filter(), None);
    assert!(app.help_visible());
    press(&mut app, KeyCode::Esc);
    assert!(!app.help_visible());
}

#[test]
fn help_pane_any_key_closes_when_not_filtering() {
    let mut app = app_with(1);
    press(&mut app, KeyCode::Char('?'));
    assert!(app.help_visible());
    // A non-f key with no active filter closes the sheet, as before.
    press(&mut app, KeyCode::Char('j'));
    assert!(!app.help_visible());
    assert_eq!(app.help_filter(), None);
}

#[test]
fn commands_tab_appears_only_when_commands_md_exists() {
    use crate::ui::Tab;

    let mut app = app_with(1);
    let id = app.selected_rca().expect("selected").id.clone();

    // No commands.md → Commands tab hidden; number keys skip it.
    assert!(!app.visible_tabs().contains(&Tab::Commands));

    // Write commands.md and reload: the tab appears.
    std::fs::write(
        app.store.workspace_dir(&id).join("commands.md"),
        "# Commands\n\n```bash\ncargo test\n```\n",
    )
    .expect("write");
    app.reload();
    assert!(
        app.visible_tabs().contains(&Tab::Commands),
        "Commands tab shows once the file exists"
    );

    // Selecting the tab loads it without error.
    app.switch_tab(Tab::Commands);
    app.ensure_pane();
    assert_eq!(app.tab(), Tab::Commands);

    // Remove it again: the tab hides and the view snaps back to a visible
    // tab instead of showing an empty Commands pane.
    std::fs::remove_file(app.store.workspace_dir(&id).join("commands.md")).expect("rm");
    app.reload();
    app.ensure_pane();
    assert!(!app.visible_tabs().contains(&Tab::Commands));
    assert_ne!(app.tab(), Tab::Commands, "snapped off the vanished tab");
}
