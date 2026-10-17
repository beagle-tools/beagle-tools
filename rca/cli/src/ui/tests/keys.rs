//! Tests for key handling and navigation (`ui::keys`).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use crossterm::event::{KeyEvent, KeyModifiers};

use crate::ui::testutil::{app_with, press};
use crate::ui::ViewportInfo;

use super::*;

use crate::agentd::{AgentSnapshot, AgentsStatus, DaemonSnapshot};

/// A `Connected` daemon status with one running agent per id.
fn connected(ids: &[&str]) -> AgentsStatus {
    AgentsStatus::Connected(DaemonSnapshot {
        version: "0.1.0".to_string(),
        agents: ids
            .iter()
            .map(|id| AgentSnapshot {
                id: (*id).to_string(),
                enabled: true,
                running: true,
                last_tick: None,
                active_sessions: 0,
                last_results: Vec::new(),
            })
            .collect(),
    })
}

#[test]
fn agents_selection_moves_and_clamps() {
    let mut app = app_with(1);
    app.set_agents_status(connected(&["a", "b", "c"]));
    press(&mut app, KeyCode::Char('A'));
    assert_eq!(app.agents_selected(), 0);
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(app.agents_selected(), 1);
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Char('j')); // clamps at the last agent
    assert_eq!(app.agents_selected(), 2);
    press(&mut app, KeyCode::Char('k'));
    press(&mut app, KeyCode::Char('k'));
    press(&mut app, KeyCode::Char('k')); // clamps at the first
    assert_eq!(app.agents_selected(), 0);
}

#[test]
fn agents_e_opens_config_for_edit() {
    let mut app = app_with(1);
    press(&mut app, KeyCode::Char('A'));
    assert!(matches!(
        press(&mut app, KeyCode::Char('e')),
        Flow::EditAgentsConfig(_)
    ));
}

#[test]
fn agents_toggle_and_log_are_safe_without_a_selection() {
    let mut app = app_with(1);
    press(&mut app, KeyCode::Char('A'));
    // Default status is Connecting → no agent selected → x and enter no-op
    // (no socket call), and the screen stays put.
    assert_eq!(press(&mut app, KeyCode::Char('x')), Flow::Continue);
    assert_eq!(press(&mut app, KeyCode::Enter), Flow::Continue);
    assert_eq!(app.screen(), Screen::Agents);
}

#[test]
fn shift_a_toggles_the_agents_screen() {
    let mut app = app_with(1);
    assert_eq!(app.screen(), Screen::Rcas);
    press(&mut app, KeyCode::Char('A'));
    assert_eq!(app.screen(), Screen::Agents);
    // `A` toggles back...
    press(&mut app, KeyCode::Char('A'));
    assert_eq!(app.screen(), Screen::Rcas);
    // ...and so does Esc from the agents screen.
    press(&mut app, KeyCode::Char('A'));
    press(&mut app, KeyCode::Esc);
    assert_eq!(app.screen(), Screen::Rcas);
}

#[test]
fn agents_screen_leaves_rca_state_untouched() {
    let mut app = app_with(3);
    let focus_before = app.focus();
    let tab_before = app.tab();
    press(&mut app, KeyCode::Char('A'));
    press(&mut app, KeyCode::Char('A'));
    assert_eq!(app.focus(), focus_before);
    assert_eq!(app.tab(), tab_before);
}

#[test]
fn agents_screen_ignores_rca_keys_but_still_quits() {
    let mut app = app_with(1);
    press(&mut app, KeyCode::Char('A'));
    // A normal RCA key is a no-op here and the screen stays put.
    assert_eq!(press(&mut app, KeyCode::Char('j')), Flow::Continue);
    assert_eq!(app.screen(), Screen::Agents);
    // Shift-Q still quits.
    assert_eq!(press(&mut app, KeyCode::Char('Q')), Flow::Quit);
}

#[test]
fn only_shift_q_and_ctrl_c_quit() {
    let mut app = app_with(1);
    assert_eq!(
        press(&mut app, KeyCode::Char('q')),
        Flow::Continue,
        "plain q must not quit"
    );
    assert_eq!(press(&mut app, KeyCode::Char('Q')), Flow::Quit);
    let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert_eq!(app.handle_key(ctrl_c), Flow::Quit);
}

#[test]
fn arrow_keys_switch_tabs_from_either_pane() {
    let mut app = app_with(1);
    press(&mut app, KeyCode::Right); // from list focus
    assert_eq!(app.tab(), Tab::Timeline);
    press(&mut app, KeyCode::Right);
    assert_eq!(app.tab(), Tab::RootCause);
    press(&mut app, KeyCode::Left); // from content focus (Right focused it)
    assert_eq!(app.tab(), Tab::Timeline);
}

#[test]
fn b_returns_focus_to_the_list() {
    let mut app = app_with(1);
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.focus(), Focus::Content);
    press(&mut app, KeyCode::Char('b'));
    assert_eq!(app.focus(), Focus::List);
}

#[test]
fn list_navigation_clamps_at_both_ends() {
    let mut app = app_with(3);
    press(&mut app, KeyCode::Char('k'));
    assert_eq!(app.selected_index(), 0, "cannot go above the first item");
    for _ in 0..10 {
        press(&mut app, KeyCode::Char('j'));
    }
    assert_eq!(app.selected_index(), 2, "cannot go past the last item");
}

#[test]
fn navigation_on_empty_store_does_not_panic() {
    let mut app = app_with(0);
    for code in [
        KeyCode::Char('j'),
        KeyCode::Enter,
        KeyCode::Tab,
        KeyCode::Char('G'),
    ] {
        press(&mut app, code);
    }
    assert!(app.selected_rca().is_none());
}

#[test]
fn tab_keys_on_an_empty_store_explain_instead_of_silence() {
    // The welcome screen has no tab bar, so a mute Tab press reads as a
    // broken keybinding. Every tab-switching key must say why instead.
    let mut app = app_with(0);
    for code in [
        KeyCode::Tab,
        KeyCode::BackTab,
        KeyCode::Right,
        KeyCode::Char('3'),
    ] {
        press(&mut app, code);
        let status = app.status_line().expect("status explains the no-op");
        assert!(status.contains("no incidents yet"), "got: {status}");
        assert_eq!(app.focus(), Focus::List, "focus stays on the list");
    }
    assert_eq!(app.tab(), Tab::Summary, "tab state unchanged");
}

#[test]
fn tab_keys_with_a_non_matching_filter_point_at_the_filter() {
    let mut app = app_with(2);
    press(&mut app, KeyCode::Char('f'));
    press(&mut app, KeyCode::Char('/')); // facet mode → typing
    press(&mut app, KeyCode::Char('z')); // matches nothing
    press(&mut app, KeyCode::Enter); // keep filter, leave search mode
    assert_eq!(app.visible_len(), 0);

    press(&mut app, KeyCode::Tab);
    let status = app.status_line().expect("status set");
    assert!(status.contains("filter"), "got: {status}");
}

#[test]
fn enter_focuses_content_and_esc_returns_to_list() {
    let mut app = app_with(1);
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.focus(), Focus::Content);
    press(&mut app, KeyCode::Esc);
    assert_eq!(app.focus(), Focus::List);
}

#[test]
fn number_keys_jump_to_tabs() {
    let mut app = app_with(1);
    press(&mut app, KeyCode::Char('3'));
    assert_eq!(app.tab(), Tab::RootCause);
    press(&mut app, KeyCode::Char('6'));
    assert_eq!(app.tab(), Tab::FinalReview);
    press(&mut app, KeyCode::Char('7'));
    assert_eq!(app.tab(), Tab::Diagrams);
}

#[test]
fn key_9_jumps_to_the_log_tab() {
    let mut app = app_with(1);
    press(&mut app, KeyCode::Char('9'));
    assert_eq!(app.tab(), Tab::Log);
    assert_eq!(Tab::Log.section(), Some(crate::model::SectionKind::Log));
}

/// Writes `body` as the selected incident's summary and reloads, so the
/// content pane has real, long lines for cursor/scroll tests.
fn set_summary(app: &mut crate::ui::App, body: &str) {
    let id = app.selected_rca().expect("selected").id.clone();
    std::fs::write(app.store.workspace_dir(&id).join("summary.md"), body).expect("write");
    app.reload();
}

#[test]
fn switching_tab_resets_scroll() {
    let mut app = app_with(1);
    let long: String = (0..40).fold(String::new(), |mut s, i| {
        use std::fmt::Write as _;
        let _ = writeln!(s, "line {i}");
        s
    });
    set_summary(&mut app, &long);
    app.viewport = ViewportInfo {
        content_lines: 40,
        height: 10,
        width: 80,
    };
    app.ensure_pane();
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Char('G')); // cursor to bottom scrolls the view
    assert!(app.scroll_offsets().0 > 0, "cursor at bottom scrolled");
    press(&mut app, KeyCode::Tab);
    assert_eq!(app.scroll_offsets().0, 0, "a new tab resets scroll");
}

#[test]
fn cursor_to_bottom_clamps_scroll_to_content() {
    let mut app = app_with(1);
    let long: String = (0..30).fold(String::new(), |mut s, i| {
        use std::fmt::Write as _;
        let _ = writeln!(s, "line {i}");
        s
    });
    set_summary(&mut app, &long);
    app.viewport = ViewportInfo {
        content_lines: 30,
        height: 10,
        width: 80,
    };
    app.ensure_pane();
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Char('G'));
    // Cursor on the last line (29); reveal scrolls to 29 - (10 - 1) = 20.
    assert_eq!(
        app.scroll_offsets().0,
        20,
        "bottom = content minus viewport"
    );
}

#[test]
fn y_in_content_yanks_the_cursor_line() {
    let mut app = app_with(1);
    set_summary(&mut app, "alpha\nbravo\ncharlie\n");
    app.viewport = ViewportInfo {
        content_lines: 3,
        height: 10,
        width: 80,
    };
    app.ensure_pane();
    press(&mut app, KeyCode::Enter); // focus content, cursor on line 0
    press(&mut app, KeyCode::Char('j')); // → line 1 ("bravo")
    assert_eq!(app.content_cursor(), 1);
    press(&mut app, KeyCode::Char('y'));
    assert!(
        app.status_line().is_some_and(|s| s.contains("copied line")),
        "y in content yanks the cursor line: {:?}",
        app.status_line()
    );
    // `Y` yanks the incident id, from either pane.
    press(&mut app, KeyCode::Char('b')); // back to the list
    press(&mut app, KeyCode::Char('Y'));
    assert!(app.status_line().is_some_and(|s| s.contains("copied id")));
}

#[test]
fn v_visual_select_extends_and_y_yanks_the_range() {
    let mut app = app_with(1);
    set_summary(&mut app, "alpha\nbravo\ncharlie\ndelta\n");
    app.viewport = ViewportInfo {
        content_lines: 4,
        height: 10,
        width: 80,
    };
    app.ensure_pane();
    press(&mut app, KeyCode::Enter); // content focus, cursor on line 0

    // v starts a selection; j extends it to line 1.
    press(&mut app, KeyCode::Char('v'));
    assert_eq!(app.selection_lines(), Some((0, 0)));
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(app.selection_lines(), Some((0, 1)), "selection extends");

    // y yanks the two-line range and ends the selection.
    press(&mut app, KeyCode::Char('y'));
    assert_eq!(app.selection_lines(), None, "yank ends the selection");
    assert!(
        app.status_line()
            .is_some_and(|s| s.contains("copied 2 lines")),
        "yanks the range: {:?}",
        app.status_line()
    );

    // v again then v cancels without yanking.
    press(&mut app, KeyCode::Char('v'));
    assert!(app.selection_lines().is_some());
    press(&mut app, KeyCode::Char('v'));
    assert_eq!(app.selection_lines(), None, "second v cancels");
}

#[test]
fn follow_mode_pins_scroll_to_the_bottom() {
    let mut app = app_with(1);
    app.viewport = ViewportInfo {
        content_lines: 100,
        height: 10,
        width: 80,
    };
    assert!(!app.follow());
    press(&mut app, KeyCode::Char('F'));
    assert!(app.follow());
    assert_eq!(
        app.scroll_offsets().0,
        u16::MAX,
        "jumps to tail (draw clamps)"
    );
    press(&mut app, KeyCode::Char('F'));
    assert!(!app.follow());
}

#[test]
fn esc_exits_follow_mode_from_either_focus() {
    let mut app = app_with(1);

    // Content focus: esc peels follow before leaving the pane.
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Char('F'));
    assert!(app.follow());
    press(&mut app, KeyCode::Esc);
    assert!(!app.follow(), "first esc exits follow");
    assert_eq!(app.focus(), Focus::Content, "still on the content");
    press(&mut app, KeyCode::Esc);
    assert_eq!(app.focus(), Focus::List, "second esc leaves the pane");

    // List focus: esc exits follow there too.
    press(&mut app, KeyCode::Char('F'));
    assert!(app.follow());
    press(&mut app, KeyCode::Esc);
    assert!(!app.follow());
}
