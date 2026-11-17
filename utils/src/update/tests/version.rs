//! Tests for versions, channels and release listing (`update::mod`).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::*;

const CHANNEL: Channel = Channel {
    repo: "acme/widgets",
    tag_prefix: "widgets-cli-v",
    asset_prefix: "widgets-cli",
    binary: "widgets",
    companions: &[],
};

fn v(s: &str) -> Version {
    s.parse().expect("valid test version")
}

#[test]
fn version_parses_with_and_without_the_v_prefix() {
    assert_eq!(v("0.2.0"), v("v0.2.0"));
    assert_eq!(v("1.12.3").to_string(), "1.12.3");
    assert_eq!(CHANNEL.parse_version("widgets-cli-v0.2.0"), Ok(v("0.2.0")));
    assert_eq!(CHANNEL.parse_version("v0.2.0"), Ok(v("0.2.0")));
    assert_eq!(CHANNEL.tag(v("0.2.0")), "widgets-cli-v0.2.0");
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
fn pkg_versions_fall_back_to_zero() {
    assert_eq!(Version::from_pkg("1.2.3"), v("1.2.3"));
    assert_eq!(Version::from_pkg("1.2.3-dev"), v("0.0.0"));
}

#[test]
fn releases_keep_only_this_channels_published_tags_newest_first() {
    let json = r#"[
        {"tag_name": "widgets-cli-v0.1.0"},
        {"tag_name": "widgets-cli-v0.3.0-rc1", "prerelease": true},
        {"tag_name": "widgets-cli-v0.4.0", "draft": true},
        {"tag_name": "widgets-cli-latest"},
        {"tag_name": "widgets-app-v9.0.0"},
        {"tag_name": "v0.7.1"},
        {"tag_name": "widgets-cli-v0.2.0"}
    ]"#;
    let versions = CHANNEL.parse_releases(json).expect("parses");
    assert_eq!(versions, [v("0.2.0"), v("0.1.0")]);
}

#[test]
fn non_array_release_bodies_error_with_context() {
    let err = CHANNEL
        .parse_releases(r#"{"message": "API rate limit exceeded"}"#)
        .expect_err("objects are not release lists");
    assert!(
        err.to_string().contains("rate limit"),
        "body surfaced: {err}"
    );
}

#[test]
fn from_tag_requires_the_channel_prefix() {
    assert_eq!(CHANNEL.from_tag("widgets-cli-v1.2.3"), Some(v("1.2.3")));
    assert_eq!(CHANNEL.from_tag("v1.2.3"), None);
    assert_eq!(CHANNEL.from_tag("widgets-cli-latest"), None);
}

#[test]
fn asset_names_and_urls_follow_the_release_contract() {
    assert_eq!(
        CHANNEL.asset_name("aarch64-apple-darwin"),
        "widgets-cli-aarch64-apple-darwin.tar.gz"
    );
    assert_eq!(
        CHANNEL.asset_url(v("0.3.0"), "widgets-cli-x.tar.gz.sha256"),
        "https://github.com/acme/widgets/releases/download/widgets-cli-v0.3.0/widgets-cli-x.tar.gz.sha256"
    );
    assert_eq!(
        CHANNEL.releases_page(),
        "https://github.com/acme/widgets/releases"
    );
}
