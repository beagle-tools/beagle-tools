//! Tests for `extension.toml` parsing and compatibility checks.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use std::path::Path;

use super::*;

const GOOD: &str = r#"
schema = 1
name = "rca"
version = "0.29.0"
protocol = 1
min_base_version = "0.1.0"
bin = "beagle-rca"
extra_bins = ["beagle-rca-daemon"]
description = "AI-first incident debugging workspaces"
post_update = ["daemon", "restart"]
"#;

fn parse(text: &str) -> Result<ExtensionManifest> {
    ExtensionManifest::parse(text, Path::new("extension.toml"))
}

#[test]
fn parses_every_field() {
    let m = parse(GOOD).unwrap();
    assert_eq!(m.name, "rca");
    assert_eq!(m.version, Version::new(0, 29, 0));
    assert_eq!(m.min_base_version, Version::new(0, 1, 0));
    assert_eq!(
        m.all_bins().collect::<Vec<_>>(),
        ["beagle-rca", "beagle-rca-daemon"]
    );
    assert_eq!(m.post_update, ["daemon", "restart"]);
}

#[test]
fn optional_fields_default_to_empty() {
    let text = GOOD
        .replace("extra_bins = [\"beagle-rca-daemon\"]\n", "")
        .replace("post_update = [\"daemon\", \"restart\"]\n", "");
    let m = parse(&text).unwrap();
    assert!(m.extra_bins.is_empty() && m.post_update.is_empty());
}

#[test]
fn unknown_field_is_rejected() {
    let err = parse(&format!("{GOOD}surprise = true\n")).unwrap_err();
    assert!(err.to_string().contains("surprise"), "{err}");
}

#[test]
fn schema_2_gets_an_upgrade_hint() {
    let err = parse(&GOOD.replace("schema = 1", "schema = 2")).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("schema 2") && msg.contains("beagle update"),
        "{msg}"
    );
    assert!(parse(&GOOD.replace("schema = 1\n", "")).is_err());
}

#[test]
fn bad_versions_and_paths_are_rejected() {
    assert!(parse(&GOOD.replace("\"0.29.0\"", "\"0.29\"")).is_err());
    for bin in ["../evil", "a/b", "", ".."] {
        let text = GOOD.replace("\"beagle-rca\"", &format!("\"{bin}\""));
        assert!(parse(&text).is_err(), "bin `{bin}` must be rejected");
    }
    assert!(parse("not = [toml").is_err());
}

#[test]
fn protocol_too_new_says_to_update() {
    let m = parse(&GOOD.replace("protocol = 1", "protocol = 2")).unwrap();
    let err = m.check_compatible(Version::new(9, 0, 0)).unwrap_err();
    assert!(
        matches!(err, Error::ProtocolTooNew { protocol: 2, .. }),
        "{err}"
    );
    assert_eq!(
        err.to_string(),
        "rca 0.29.0 needs a newer beagle (protocol 2) — run: beagle update"
    );
}

#[test]
fn min_base_version_is_enforced() {
    let m = parse(&GOOD.replace("\"0.1.0\"", "\"0.3.0\"")).unwrap();
    assert!(matches!(
        m.check_compatible(Version::new(0, 2, 9)),
        Err(Error::BaseTooOld { .. })
    ));
    assert!(m.check_compatible(Version::new(0, 3, 0)).is_ok());
}

#[test]
fn name_mismatch_is_reported() {
    let m = parse(&GOOD.replace("name = \"rca\"", "name = \"agentd\"")).unwrap();
    let err = m.check_name("rca").unwrap_err();
    assert_eq!(
        err.to_string(),
        "tarball for \"rca\" declares name \"agentd\""
    );
    assert!(parse(GOOD).unwrap().check_name("rca").is_ok());
}
