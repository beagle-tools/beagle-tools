//! Tests for the update-check cache and notice rate limiting.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::*;

fn v(s: &str) -> Version {
    s.parse().unwrap()
}

#[test]
fn round_trips_through_disk() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("nested/state.toml");
    let mut state = State::default();
    state.record_check("rca", Some(v("0.30.0")), 100);
    state.mark_notified("rca", 50);
    state.save(&path).unwrap();
    assert_eq!(State::load(&path), state);
}

#[test]
fn missing_or_corrupt_files_load_empty() {
    let tmp = tempfile::tempdir().unwrap();
    assert_eq!(State::load(&tmp.path().join("nope.toml")), State::default());
    let bad = tmp.path().join("bad.toml");
    std::fs::write(&bad, "this is [not toml").unwrap();
    assert_eq!(State::load(&bad), State::default());
}

#[test]
fn notice_only_for_a_newer_version_at_most_daily() {
    let mut state = State::default();
    state.record_check("rca", Some(v("0.30.0")), 0);
    let day = NOTICE_INTERVAL_SECS;
    assert_eq!(state.due_notice("rca", v("0.29.0"), day), Some(v("0.30.0")));
    assert_eq!(
        state.due_notice("rca", v("0.30.0"), day),
        None,
        "up to date"
    );
    assert_eq!(
        state.due_notice("agentd", v("0.1.0"), day),
        None,
        "never checked"
    );
    state.mark_notified("rca", day);
    assert_eq!(state.due_notice("rca", v("0.29.0"), day + day - 1), None);
    assert_eq!(
        state.due_notice("rca", v("0.29.0"), day + day),
        Some(v("0.30.0"))
    );
}

#[test]
fn notices_respect_the_opt_outs() {
    assert!(notices_enabled(None, None));
    assert!(!notices_enabled(Some("1"), None));
    assert!(!notices_enabled(None, Some("true")));
    assert!(notices_enabled(Some("0"), None));
}
