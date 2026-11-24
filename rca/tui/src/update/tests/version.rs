//! Tests for rca's release channel (`update::mod`). Version parsing,
//! listing and the install pipeline themselves are tested in
//! `beagle_utils::update`; these pin rca's half of the release contract.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::*;

fn v(s: &str) -> Version {
    s.parse().expect("valid test version")
}

#[test]
fn version_parses_with_and_without_the_v_prefix() {
    assert_eq!(parse_version("0.2.0"), Ok(v("0.2.0")));
    assert_eq!(parse_version("v0.2.0"), Ok(v("0.2.0")));
    assert_eq!(parse_version("beagle-rca-v0.2.0"), Ok(v("0.2.0")));
    assert_eq!(channel().tag(v("0.2.0")), "beagle-rca-v0.2.0");
    assert!(parse_version("0.2.0-rc1").is_err());
}

#[test]
fn current_version_matches_the_crate() {
    assert_eq!(current().to_string(), env!("CARGO_PKG_VERSION"));
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
        {"tag_name": "beagle-cli-v1.0.0"},
        {"tag_name": "beagle-workflows-v1.0.0"},
        {"tag_name": "v0.7.1"},
        {"tag_name": "desktop-v0.2.0"},
        {"tag_name": "beagle-rca-v0.30.0"}
    ]"#;
    let releases = channel().parse_releases(json).expect("parses");
    assert_eq!(releases, [v("0.30.1"), v("0.30.0")]);
}

#[test]
fn asset_names_and_urls_follow_the_release_contract() {
    let channel = channel();
    assert_eq!(
        channel.asset_name("aarch64-apple-darwin"),
        "beagle-rca-aarch64-apple-darwin.tar.gz"
    );
    assert_eq!(
        channel.asset_url(
            v("0.30.0"),
            "beagle-rca-x86_64-unknown-linux-musl.tar.gz.sha256"
        ),
        "https://github.com/beagle-tools/beagle-tools/releases/download/\
         beagle-rca-v0.30.0/beagle-rca-x86_64-unknown-linux-musl.tar.gz.sha256"
    );
    assert_eq!(channel.binary, "beagle");
    assert_eq!(channel.companions, ["beagle-agentd"]);
}

#[test]
fn shared_errors_map_onto_rca_errors() {
    let err: Error = beagle_utils::update::Error::UnsupportedTarget.into();
    assert!(
        err.to_string().contains("cargo install --git"),
        "keeps the install-from-source hint: {err}"
    );
}
