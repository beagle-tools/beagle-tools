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
fn under_beagle_update_and_version_list_are_not_commands() {
    let err = Mode::Unified
        .allows(Command::Update { version: None })
        .expect_err("no rca-only self-update under `beagle`");
    assert!(
        err.contains("unknown command `update`") && err.contains("`beagle update`"),
        "{err}"
    );
    let err = Mode::Unified
        .allows(Command::VersionList)
        .expect_err("no rca-only version list under `beagle`");
    assert!(err.contains("unknown command `version list`"), "{err}");
    // The standalone binary still updates itself, and other commands pass.
    assert!(Mode::Standalone.allows(Command::VersionList).is_ok());
    assert!(Mode::Unified.allows(Command::Help).is_ok());
}

#[test]
fn errors_start_with_the_command_typed() {
    assert_eq!(Mode::Unified.prefix(), "beagle rca");
    assert_eq!(Mode::Standalone.prefix(), "beagle");
}

#[test]
fn the_help_speaks_the_unified_spelling() {
    assert!(USAGE.starts_with("beagle rca"));
    for line in USAGE.lines().filter(|l| l.starts_with("    beagle ")) {
        assert!(line.starts_with("    beagle rca"), "{line}");
    }
}

#[test]
fn a_broken_config_warns_and_rca_still_starts() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("rca.toml");
    std::fs::write(&path, "rooot = \"/x\"\n").unwrap();
    let (config, warning) = config_or_warning(config::load(&path));
    assert!(config.is_none());
    let warning = warning.unwrap();
    assert!(warning.contains("rca.toml"), "{warning}");
    assert!(warning.contains("using defaults"), "{warning}");
}
