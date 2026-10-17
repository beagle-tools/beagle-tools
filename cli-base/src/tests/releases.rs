//! Tests for release listing and selection, from fixture JSON.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::*;
use crate::registry::{find, REGISTRY};

const T: &str = "aarch64-apple-darwin";

const FIXTURE: &str = r#"[
  {"tag_name": "beagle-rca-v0.30.0", "draft": true, "assets": [{"name": "beagle-rca-ext-aarch64-apple-darwin.tar.gz"}]},
  {"tag_name": "beagle-rca-v0.29.1", "prerelease": true, "assets": [{"name": "beagle-rca-ext-aarch64-apple-darwin.tar.gz"}]},
  {"tag_name": "beagle-rca-v0.29.0", "assets": [{"name": "beagle-rca-ext-aarch64-apple-darwin.tar.gz"}]},
  {"tag_name": "beagle-tandem-v9.9.9", "assets": []},
  {"tag_name": "beagle-rca-vgarbage", "assets": []},
  {"tag_name": "beagle-rca-v0.28.0", "assets": [{"name": "beagle-rca-ext-aarch64-apple-darwin.tar.gz"}]},
  {"tag_name": "beagle-rca-v0.27.0", "assets": [{"name": "beagle-rca-aarch64-apple-darwin.tar.gz"}]}
]"#;

fn rca() -> &'static ExtensionSpec {
    find(REGISTRY, "rca").unwrap()
}

fn versions(releases: &[Release]) -> Vec<String> {
    releases.iter().map(|r| r.version.to_string()).collect()
}

#[test]
fn filters_by_prefix_and_skips_drafts_prereleases_and_garbage() {
    let releases = parse_releases(FIXTURE, "beagle-rca-v", false).unwrap();
    assert_eq!(versions(&releases), ["0.29.0", "0.28.0", "0.27.0"]);
    assert_eq!(releases[0].tag, "beagle-rca-v0.29.0");
}

#[test]
fn pre_includes_prereleases_but_never_drafts() {
    let releases = parse_releases(FIXTURE, "beagle-rca-v", true).unwrap();
    assert_eq!(
        versions(&releases),
        ["0.29.1", "0.29.0", "0.28.0", "0.27.0"]
    );
    assert!(releases[0].prerelease);
}

#[test]
fn rejects_a_non_array_body() {
    let err = parse_releases(r#"{"message": "rate limited"}"#, "beagle-rca-v", false).unwrap_err();
    assert!(matches!(err, Error::ParseReleases(_)), "{err}");
}

#[test]
fn choose_defaults_to_the_newest_release_with_an_asset() {
    let releases = parse_releases(FIXTURE, "beagle-rca-v", false).unwrap();
    assert_eq!(
        choose(&releases, rca(), T, None).unwrap().tag,
        "beagle-rca-v0.29.0"
    );
    // 0.27.0 only has the legacy (non `-ext-`) asset.
    assert_eq!(installable(&releases, rca(), T).len(), 2);
}

#[test]
fn choose_honours_a_requested_version_in_any_form() {
    let releases = parse_releases(FIXTURE, "beagle-rca-v", false).unwrap();
    for input in ["0.28.0", "v0.28.0", "beagle-rca-v0.28.0"] {
        assert_eq!(
            choose(&releases, rca(), T, Some(input))
                .unwrap()
                .version
                .to_string(),
            "0.28.0"
        );
    }
}

#[test]
fn unknown_version_lists_the_nearest() {
    let releases = parse_releases(FIXTURE, "beagle-rca-v", false).unwrap();
    let err = choose(&releases, rca(), T, Some("0.1.0")).unwrap_err();
    assert_eq!(
        err.to_string(),
        "rca 0.1.0 is not released for aarch64-apple-darwin; nearest: 0.28.0, 0.29.0"
    );
}

#[test]
fn no_build_for_the_target_is_unsupported() {
    let releases = parse_releases(FIXTURE, "beagle-rca-v", false).unwrap();
    let err = choose(&releases, rca(), "x86_64-unknown-linux-musl", None).unwrap_err();
    assert_eq!(
        err.to_string(),
        "no rca release has a build for x86_64-unknown-linux-musl yet — see: beagle extensions available"
    );
}

#[test]
fn asset_urls_join_base_tag_and_asset() {
    let source = ReleaseSource {
        releases_url: String::new(),
        download_base: "file:///srv/dl/".to_owned(),
    };
    assert_eq!(
        source.asset_url("t", "a.tar.gz"),
        "file:///srv/dl/t/a.tar.gz"
    );
}
