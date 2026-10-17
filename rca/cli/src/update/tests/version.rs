//! Tests for version parsing and release listing (`update::mod`).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::*;

fn v(s: &str) -> Version {
    s.parse().expect("valid test version")
}

#[test]
fn version_parses_with_and_without_the_v_prefix() {
    assert_eq!(v("0.2.0"), v("v0.2.0"));
    assert_eq!(v("0.2.0"), v("beagle-rca-v0.2.0"));
    assert_eq!(v("1.12.3").to_string(), "1.12.3");
    assert_eq!(v("0.2.0").tag(), "beagle-rca-v0.2.0");
}

#[test]
fn version_rejects_garbage() {
    for bad in [
        "",
        "1",
        "1.2",
        "1.2.3.4",
        "a.b.c",
        "1.2.x",
        "v",
        "0.2.0-rc1",
    ] {
        assert!(bad.parse::<Version>().is_err(), "`{bad}` must be rejected");
    }
}

#[test]
fn versions_order_newest_greatest() {
    assert!(v("0.10.0") > v("0.9.9"), "numeric, not lexicographic");
    assert!(v("1.0.0") > v("0.99.99"));
    assert!(v("0.2.1") > v("0.2.0"));
}

#[test]
fn current_version_matches_the_crate() {
    assert_eq!(Version::current().to_string(), env!("CARGO_PKG_VERSION"));
}

#[test]
fn releases_parse_filter_and_sort_newest_first() {
    let json = r#"[
        {"tag_name": "beagle-rca-v0.1.0"},
        {"tag_name": "beagle-rca-v0.3.0-rc1", "prerelease": true},
        {"tag_name": "beagle-rca-v0.4.0", "draft": true},
        {"tag_name": "weird-tag"},
        {"tag_name": "beagle-rca-v0.2.0"}
    ]"#;
    let releases = parse_releases(json).expect("parses");
    let tags: Vec<String> = releases.iter().map(|r| r.version.tag()).collect();
    assert_eq!(tags, ["beagle-rca-v0.2.0", "beagle-rca-v0.1.0"]);
}

#[test]
fn non_array_release_bodies_error_with_context() {
    let err = parse_releases(r#"{"message": "API rate limit exceeded"}"#)
        .expect_err("objects are not release lists");
    assert!(
        err.to_string().contains("rate limit"),
        "body surfaced: {err}"
    );
}

#[test]
fn repo_slug_is_owner_slash_name() {
    assert_eq!(repo_slug(), "beagle-tools/beagle-tools");
}

#[test]
fn parse_releases_keeps_only_rca_tags_from_the_monorepo() {
    // The monorepo publishes every app's releases in one list, plus the
    // rolling `-latest` mirrors and legacy `v*` tags; only `beagle-rca-v*`
    // versions are installable by this CLI.
    let json = r#"[
        {"tag_name": "beagle-tandem-v0.9.0"},
        {"tag_name": "beagle-rca-v0.30.1"},
        {"tag_name": "beagle-rca-latest"},
        {"tag_name": "beagle-workflows-v1.0.0"},
        {"tag_name": "v0.7.1"},
        {"tag_name": "desktop-v0.2.0"},
        {"tag_name": "beagle-rca-v0.30.0"}
    ]"#;
    let releases = parse_releases(json).expect("parses");
    let tags: Vec<String> = releases.iter().map(|r| r.version.tag()).collect();
    assert_eq!(tags, ["beagle-rca-v0.30.1", "beagle-rca-v0.30.0"]);
}

#[test]
fn from_tag_requires_the_rca_prefix() {
    assert_eq!(Version::from_tag("beagle-rca-v1.2.3"), Some(v("1.2.3")));
    assert_eq!(Version::from_tag("v1.2.3"), None);
    assert_eq!(Version::from_tag("beagle-tandem-v1.2.3"), None);
    assert_eq!(Version::from_tag("beagle-rca-latest"), None);
}

#[test]
fn asset_names_and_urls_follow_the_release_contract() {
    assert_eq!(
        asset_name("aarch64-apple-darwin"),
        "beagle-rca-aarch64-apple-darwin.tar.gz"
    );
    assert_eq!(
        asset_url(
            v("0.30.0"),
            "beagle-rca-x86_64-unknown-linux-musl.tar.gz.sha256"
        ),
        "https://github.com/beagle-tools/beagle-tools/releases/download/\
         beagle-rca-v0.30.0/beagle-rca-x86_64-unknown-linux-musl.tar.gz.sha256"
    );
}
