//! Tests for the extension registry, including its sync with the release
//! scripts (`scripts/release/apps.sh`).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use std::collections::{HashMap, HashSet};

use super::*;

#[test]
fn names_are_unique_non_reserved_and_have_targets() {
    let mut seen = HashSet::new();
    for spec in REGISTRY {
        assert!(!is_reserved(spec.name), "{} is reserved", spec.name);
        assert!(seen.insert(spec.name), "{} listed twice", spec.name);
        assert!(!spec.targets.is_empty(), "{} has no targets", spec.name);
    }
}

#[test]
fn aliases_collide_with_nothing() {
    let names: HashSet<&str> = REGISTRY.iter().map(|s| s.name).collect();
    let mut aliases = HashSet::new();
    for spec in REGISTRY {
        for alias in spec.aliases {
            assert!(!names.contains(alias), "alias {alias} shadows a name");
            assert!(!is_reserved(alias), "alias {alias} is reserved");
            assert!(aliases.insert(*alias), "alias {alias} used twice");
        }
    }
}

#[test]
fn find_resolves_names_and_aliases() {
    const TEST: &[ExtensionSpec] = &[ExtensionSpec {
        name: "new",
        aliases: &["old"],
        release_app: "x",
        tag_prefix: "beagle-x-v",
        asset_prefix: "beagle-x-ext",
        targets: &["t"],
        description: "",
    }];
    assert_eq!(find(TEST, "new").map(|s| s.name), Some("new"));
    assert_eq!(find(TEST, "old").map(|s| s.name), Some("new"));
    assert!(find(TEST, "nope").is_none());
    assert!(find(REGISTRY, "rca").is_some());
}

#[test]
fn reserved_names_are_reserved() {
    for name in ["extensions", "ext", "update", "version", "help", "doctor"] {
        assert!(is_reserved(name), "{name}");
    }
    assert!(!is_reserved("rca"));
}

/// Each `<app>)` block of `app_meta` in apps.sh as `KEY -> value`.
fn apps_sh_blocks() -> HashMap<String, HashMap<String, String>> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../scripts/release/apps.sh");
    let text = std::fs::read_to_string(path).expect("read apps.sh");
    let mut blocks = HashMap::new();
    let mut current: Option<(String, HashMap<String, String>)> = None;
    // Only `app_meta() { … }`: later functions reuse the same case labels.
    let body = text
        .lines()
        .skip_while(|l| !l.starts_with("app_meta()"))
        .take_while(|l| *l != "}");
    for line in body.map(str::trim) {
        if let Some(app) = line
            .strip_suffix(')')
            .filter(|a| !a.is_empty() && a.bytes().all(|b| b.is_ascii_lowercase() || b == b'-'))
        {
            current = Some((app.to_owned(), HashMap::new()));
        } else if line == ";;" {
            if let Some((app, vars)) = current.take() {
                blocks.insert(app, vars);
            }
        } else if let Some((_, vars)) = current.as_mut() {
            if let Some((key, value)) = line.split_once('=') {
                vars.insert(key.to_owned(), value.trim_matches('"').to_owned());
            }
        }
    }
    blocks
}

#[test]
fn registry_matches_the_release_scripts() {
    let blocks = apps_sh_blocks();
    for spec in REGISTRY.iter().chain([&BASE]) {
        let vars = blocks
            .get(spec.release_app)
            .unwrap_or_else(|| panic!("apps.sh has no `{})` block", spec.release_app));
        assert_eq!(
            spec.tag_prefix,
            format!("beagle-{}-v", spec.release_app),
            "{}: tag prefix (scripts/release/lib.mjs tagFor)",
            spec.name
        );
        let key = if spec.name == BASE.name {
            "APP_ASSET_PREFIX"
        } else {
            "APP_EXT_ASSET"
        };
        assert_eq!(
            vars.get(key).map(String::as_str),
            Some(spec.asset_prefix),
            "{}: {key} in apps.sh",
            spec.name
        );
        // Apps that already ship CLI tarballs must publish every target the
        // registry promises; tandem has none until its extension lands.
        if let Some(targets) = vars.get("APP_CLI_TARGETS").filter(|t| !t.is_empty()) {
            let published: Vec<&str> = targets.split_whitespace().collect();
            assert_eq!(published, spec.targets, "{}: targets", spec.name);
        }
    }
}
