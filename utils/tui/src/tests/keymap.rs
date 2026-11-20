//! Tests for `keymap`: the data matches `standards/tui-standards.json`.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::*;

const JSON: &str = include_str!("../../../../standards/tui-standards.json");

#[test]
fn every_job_has_a_row() {
    for job in [
        Job::Move,
        Job::OpenBack,
        Job::Tabs,
        Job::HelpQuit,
        Job::Reload,
        Job::Search,
        Job::OpenInBrowser,
        Job::Edit,
        Job::Copy,
        Job::Resize,
    ] {
        assert!(!binding(job).keys.is_empty(), "{job:?}");
    }
}

#[test]
fn every_key_spec_parses() {
    for b in STANDARD {
        for spec in b.keys {
            assert_ne!(key(spec).code, KeyCode::Null, "{}: {spec}", b.name);
        }
    }
    assert_eq!(
        key("ctrl-d"),
        KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL)
    );
}

#[test]
fn the_json_lists_the_same_rows_in_the_same_order() {
    let rows: Vec<&str> = JSON
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("{\"job\""))
        .map(|l| l.trim_end_matches(','))
        .collect();
    let ours: Vec<String> = STANDARD.iter().map(json_line).collect();
    assert_eq!(rows, ours, "standards/tui-standards.json keymap drifted");
}

#[test]
fn the_json_lists_the_same_github_writes() {
    for (key, action) in GITHUB_WRITES {
        let line = format!("{{\"key\": \"{key}\", \"action\": \"{action}\"}}");
        assert!(JSON.contains(&line), "missing {line}");
    }
    assert_eq!(
        JSON.matches("{\"key\": ").count(),
        GITHUB_WRITES.len(),
        "standards/tui-standards.json has extra GitHub writes"
    );
}
