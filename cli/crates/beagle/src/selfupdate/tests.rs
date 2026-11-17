//! Tests for `beagle update` / `beagle version` argument handling and the
//! unified release channel. The download pipeline itself is tested in
//! `beagle_utils::update`.

use super::*;

fn os(args: &[&str]) -> Vec<OsString> {
    args.iter().map(OsString::from).collect()
}

fn v(s: &str) -> Version {
    channel()
        .parse_version(s)
        .unwrap_or_else(|e| unreachable!("{e}"))
}

#[test]
fn update_parses_check_version_and_help() {
    assert_eq!(parse_update(&os(&[])), Ok(UpdateRequest::Install(None)));
    assert_eq!(parse_update(&os(&["--check"])), Ok(UpdateRequest::Check));
    assert_eq!(parse_update(&os(&["--help"])), Ok(UpdateRequest::Help));
    assert_eq!(
        parse_update(&os(&["--version", "beagle-cli-v0.2.0"])),
        Ok(UpdateRequest::Install(Some(v("0.2.0"))))
    );
    assert_eq!(
        parse_update(&os(&["--version", "v0.2.0"])),
        Ok(UpdateRequest::Install(Some(v("0.2.0"))))
    );
}

#[test]
fn update_rejects_bad_input() {
    assert!(parse_update(&os(&["--version"])).is_err(), "missing value");
    assert!(parse_update(&os(&["--version", "latest"])).is_err());
    assert!(parse_update(&os(&["--force"])).is_err());
    assert!(parse_update(&os(&["--check", "--version", "0.1.0"])).is_err());
}

#[test]
fn the_channel_is_the_release_contract() {
    let channel = channel();
    assert_eq!(channel.repo, "beagle-tools/beagle-tools");
    assert_eq!(channel.tag(v("0.1.0")), "beagle-cli-v0.1.0");
    assert_eq!(
        channel.asset_name("x86_64-unknown-linux-musl"),
        "beagle-cli-x86_64-unknown-linux-musl.tar.gz"
    );
    assert_eq!(channel.binary, "beagle");
    // Other apps' tags (the standalone rca binary is also called `beagle`)
    // are never offered.
    assert_eq!(channel.from_tag("beagle-rca-v0.29.0"), None);
}

#[test]
fn source_builds_print_the_rebuild_command_instead_of_downloading() {
    // A source build without --version touches nothing and needs no network.
    let exe = Path::new("/nonexistent/bin/beagle");
    assert_eq!(update(&UpdateRequest::Install(None), exe), Ok(()));
}

#[test]
fn the_no_release_message_says_what_it_looked_for() {
    let message = no_release_yet();
    assert!(message.contains("beagle-cli-v*"), "{message}");
    assert!(message.contains(&current().to_string()), "{message}");
}
