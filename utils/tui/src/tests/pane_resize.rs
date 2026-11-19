//! Tests for `pane_resize`.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::*;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn third() -> SideResize {
    SideResize::new(
        "list",
        DefaultWidth::Fraction {
            divisor: 3,
            min: 30,
            max: 56,
        },
    )
}

#[test]
fn a_fraction_default_is_clamped_to_its_range() {
    let pane = third();
    assert_eq!(pane.width(120), 40);
    assert_eq!(pane.width(300), 56);
    assert_eq!(pane.width(80), 30);
}

#[test]
fn a_fixed_default_still_leaves_the_other_pane_its_minimum() {
    let pane = SideResize::new("sidebar", DefaultWidth::Fixed(32));
    assert_eq!(pane.width(200), 32);
    assert_eq!(pane.width(70), 30);
    assert_eq!(pane.last_total(), 70);
}

#[test]
fn a_tiny_terminal_keeps_the_side_minimum() {
    let pane = third();
    assert_eq!(pane.width(10), 20);
    assert_eq!(pane.width(0), 20);
}

#[test]
fn arrows_and_hl_step_the_width() {
    let mut pane = third();
    pane.start();
    let out = pane.handle_key(key(KeyCode::Right), 120);
    assert_eq!(
        out,
        Outcome {
            left: false,
            status: None
        }
    );
    assert_eq!(pane.width(120), 42);
    pane.handle_key(key(KeyCode::Char('l')), 120);
    assert_eq!(pane.width(120), 44);
    pane.handle_key(key(KeyCode::Char('h')), 120);
    pane.handle_key(key(KeyCode::Left), 120);
    assert_eq!(pane.width(120), 40);
}

#[test]
fn narrowing_stops_at_the_minimum_and_says_so() {
    let mut pane = third();
    pane.start();
    let mut last = None;
    for _ in 0..40 {
        last = pane.handle_key(key(KeyCode::Left), 200).status;
    }
    assert_eq!(pane.width(200), 20);
    assert_eq!(
        last.as_deref(),
        Some("list is at its narrowest (20 columns)")
    );
    assert!(pane.is_active());
}

#[test]
fn widening_stops_at_the_maximum_and_at_the_other_panes_minimum() {
    let mut pane = third();
    pane.start();
    for _ in 0..80 {
        pane.handle_key(key(KeyCode::Right), 400);
    }
    assert_eq!(pane.width(400), 100);
    let out = pane.handle_key(key(KeyCode::Right), 400);
    assert_eq!(
        out.status.as_deref(),
        Some("list is at its widest (100 columns)")
    );

    let mut mid = third();
    mid.start();
    for _ in 0..80 {
        mid.handle_key(key(KeyCode::Right), 100);
    }
    assert_eq!(mid.width(100), 60);
}

#[test]
fn zero_restores_the_default() {
    let mut pane = third();
    pane.start();
    pane.handle_key(key(KeyCode::Right), 120);
    pane.handle_key(key(KeyCode::Right), 120);
    let out = pane.handle_key(key(KeyCode::Char('0')), 120);
    assert!(!out.left);
    assert_eq!(
        out.status.as_deref(),
        Some("list back to its default (40 columns)")
    );
    assert_eq!(pane.width(120), 40);
    // The default follows the screen again after a reset.
    assert_eq!(pane.width(300), 56);
}

#[test]
fn esc_enter_w_and_q_all_leave() {
    for code in [
        KeyCode::Esc,
        KeyCode::Enter,
        KeyCode::Char('w'),
        KeyCode::Char('q'),
    ] {
        let mut pane = third();
        pane.start();
        let out = pane.handle_key(key(code), 120);
        assert!(out.left, "{code:?} should leave");
        assert!(!pane.is_active());
        assert_eq!(out.status.as_deref(), Some("list width 40"));
    }
}

#[test]
fn other_keys_are_swallowed() {
    let mut pane = third();
    pane.start();
    for code in [
        KeyCode::Char('p'),
        KeyCode::Char('A'),
        KeyCode::Up,
        KeyCode::Tab,
    ] {
        let out = pane.handle_key(key(code), 120);
        assert_eq!(
            out,
            Outcome {
                left: false,
                status: None
            }
        );
    }
    assert!(pane.is_active());
    assert_eq!(pane.width(120), 40);
}

#[test]
fn the_chosen_width_is_held_to_a_smaller_screen() {
    let mut pane = third();
    pane.start();
    for _ in 0..20 {
        pane.handle_key(key(KeyCode::Right), 300);
    }
    assert_eq!(pane.width(300), 96);
    assert_eq!(pane.width(90), 50);
    pane.stop();
    assert!(!pane.is_active());
}
