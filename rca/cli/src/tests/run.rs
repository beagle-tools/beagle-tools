//! Tests for the CLI entry points (`run`): which binary owns updating, and
//! the `rca` spelling the standalone binary accepts.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::*;

fn os(args: &[&str]) -> Vec<OsString> {
    args.iter().map(OsString::from).collect()
}

#[test]
fn standalone_accepts_the_unified_spelling() {
    assert_eq!(strip_rca(os(&["rca", "list"])), os(&["list"]));
    assert_eq!(strip_rca(os(&["list"])), os(&["list"]));
    assert_eq!(strip_rca(os(&["rca"])), os(&[]));
    // Only one leading `rca` goes: anything after it is rca's own argument.
    assert_eq!(strip_rca(os(&["rca", "rca"])), os(&["rca"]));
}

#[test]
fn unified_mode_defers_updating_to_beagle_update() {
    let err = execute(Command::Update { version: None }, Mode::Unified)
        .expect_err("no rca-only self-update under `beagle`");
    assert!(err.to_string().contains("run `beagle update`"), "{err}");

    let err = execute(Command::VersionList, Mode::Unified)
        .expect_err("no rca-only version list under `beagle`");
    assert!(
        err.to_string().contains("run `beagle version list`"),
        "{err}"
    );
}

#[test]
fn the_help_speaks_the_unified_spelling() {
    assert!(USAGE.starts_with("beagle rca"));
    for line in USAGE.lines().filter(|l| l.starts_with("    beagle ")) {
        assert!(line.starts_with("    beagle rca"), "{line}");
    }
}
