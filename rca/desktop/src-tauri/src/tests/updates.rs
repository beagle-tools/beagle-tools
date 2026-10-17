//! Tests for `updates`: tag parsing and release-feed filtering (the parts
//! that decide what the version picker offers).
#![allow(clippy::expect_used)] // panicking is the correct failure mode in tests

use super::*;

#[test]
fn accepts_rca_release_tags() {
    assert_eq!(tag_version("beagle-rca-v0.29.0"), Some("0.29.0"));
    assert_eq!(tag_version("beagle-rca-v10.20.30"), Some("10.20.30"));
}

#[test]
fn rejects_other_apps_mirrors_and_legacy_tags() {
    for tag in [
        "beagle-rca-latest",
        "beagle-tandem-v1.2.3",
        "beagle-workflows-v1.2.3",
        "desktop-v0.2.0",
        "v0.29.0",
        "beagle-rca-v1.2",
        "beagle-rca-v1.2.3.4",
        "beagle-rca-v1..3",
        "beagle-rca-v1.2.x",
        "beagle-rca-v1.2.3-rc1",
        "beagle-rca-v+1.2.3",
    ] {
        assert_eq!(tag_version(tag), None, "{tag}");
    }
}

#[test]
fn feed_keeps_published_rca_releases_in_order() {
    let json = r#"[
        {"tag_name": "beagle-rca-latest", "published_at": "2026-10-03T00:00:00Z",
         "assets": [{"name": "latest.json"}]},
        {"tag_name": "beagle-tandem-v0.4.0", "published_at": "2026-10-02T00:00:00Z",
         "assets": [{"name": "latest.json"}]},
        {"tag_name": "beagle-rca-v0.31.0", "draft": true, "assets": []},
        {"tag_name": "beagle-rca-v0.30.1", "published_at": "2026-10-02T00:00:00Z",
         "assets": [{"name": "Beagle-macos-arm64.dmg"}, {"name": "latest.json"}]},
        {"tag_name": "beagle-rca-v0.30.0-rc1", "prerelease": true, "assets": []},
        {"tag_name": "beagle-rca-v0.30.0", "published_at": null,
         "assets": [{"name": "beagle-rca-aarch64-apple-darwin.tar.gz"}]}
    ]"#;
    let releases = parse_releases(json).expect("parses");
    assert_eq!(
        releases,
        [
            AppRelease {
                version: "0.30.1".to_owned(),
                tag: "beagle-rca-v0.30.1".to_owned(),
                published_at: Some("2026-10-02T00:00:00Z".to_owned()),
                installable: true,
            },
            AppRelease {
                version: "0.30.0".to_owned(),
                tag: "beagle-rca-v0.30.0".to_owned(),
                published_at: None,
                installable: false,
            },
        ]
    );
}

#[test]
fn non_array_feeds_are_an_update_error() {
    let err = parse_releases(r#"{"message": "API rate limit exceeded"}"#)
        .expect_err("objects are not release lists");
    assert!(matches!(err, Error::Update(_)), "{err}");
}

#[test]
fn manifest_lives_in_the_versioned_release() {
    assert_eq!(
        manifest_url("beagle-rca-v0.30.0"),
        "https://github.com/beagle-tools/beagle-tools/releases/download/beagle-rca-v0.30.0/latest.json"
    );
}
