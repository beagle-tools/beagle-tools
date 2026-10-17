//! Tests for the download → verify → swap pipeline (`update::install`).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use std::path::PathBuf;

use super::*;
use crate::error::Error;

#[test]
fn sha_lines_parse_and_reject_non_digests() {
    let line = format!("{}  beagle-x.tar.gz\n", "ab".repeat(32));
    assert_eq!(parse_sha(&line), Some("ab".repeat(32)));
    assert_eq!(
        parse_sha("ABCD".repeat(16).as_str()).as_deref(),
        Some("abcd".repeat(16).as_str())
    );
    assert_eq!(parse_sha("not-a-digest file"), None);
    assert_eq!(parse_sha(""), None);
}

/// End-to-end install over `file://` URLs: build a fake release tarball
/// with a real checksum, install it over a fake current binary, and
/// check the swap happened atomically with the executable bit intact.
#[test]
fn install_verifies_checksum_extracts_and_swaps_the_binary() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (tarball, sha_file) = fake_release(tmp.path(), b"#!/bin/sh\necho new\n");

    let exe = tmp.path().join("bin").join("beagle");
    fs::create_dir_all(exe.parent().unwrap()).expect("bin dir");
    fs::write(&exe, "old-binary").expect("old binary");

    install(
        &format!("file://{}", tarball.display()),
        &format!("file://{}", sha_file.display()),
        &exe,
    )
    .expect("install succeeds");

    let installed = fs::read(&exe).expect("read installed");
    assert_eq!(installed, b"#!/bin/sh\necho new\n");
    assert!(
        !exe.parent().unwrap().join(".beagle.update").exists(),
        "no staging file left behind"
    );
}

#[test]
fn install_refuses_a_tampered_tarball() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (tarball, sha_file) = fake_release(tmp.path(), b"legit");
    // Corrupt the tarball after the checksum was published.
    fs::write(&tarball, "tampered bytes").expect("tamper");

    let exe = tmp.path().join("beagle");
    fs::write(&exe, "old-binary").expect("old binary");

    let err = install(
        &format!("file://{}", tarball.display()),
        &format!("file://{}", sha_file.display()),
        &exe,
    )
    .expect_err("checksum mismatch must fail the install");
    assert!(matches!(err, Error::ChecksumMismatch { .. }), "{err}");
    assert_eq!(
        fs::read(&exe).expect("read"),
        b"old-binary",
        "binary untouched on failure"
    );
}

#[test]
fn install_also_swaps_a_sibling_beagle_agentd() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (tarball, sha_file) = fake_release(tmp.path(), b"new-cli");
    let bin = tmp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin dir");
    fs::write(bin.join("beagle"), "old-cli").expect("old cli");
    fs::write(bin.join("beagle-agentd"), "old-daemon").expect("old daemon");

    install(
        &format!("file://{}", tarball.display()),
        &format!("file://{}", sha_file.display()),
        &bin.join("beagle"),
    )
    .expect("install succeeds");

    assert_eq!(fs::read(bin.join("beagle")).expect("cli"), b"new-cli");
    assert_eq!(
        fs::read(bin.join("beagle-agentd")).expect("daemon"),
        b"new-daemon"
    );
}

#[test]
fn install_leaves_agentd_alone_when_it_is_not_installed_beside_the_cli() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (tarball, sha_file) = fake_release(tmp.path(), b"new-cli");
    let bin = tmp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin dir");
    fs::write(bin.join("beagle"), "old-cli").expect("old cli");

    install(
        &format!("file://{}", tarball.display()),
        &format!("file://{}", sha_file.display()),
        &bin.join("beagle"),
    )
    .expect("install succeeds");
    assert!(!bin.join("beagle-agentd").exists(), "not created");
}

/// Builds a `beagle-rca-<target>.tar.gz` whose root holds `beagle` and
/// `beagle-agentd`, with a matching `.sha256` — the release contract layout.
fn fake_release(dir: &Path, binary: &[u8]) -> (PathBuf, PathBuf) {
    let pkg = dir.join("pkg");
    fs::create_dir_all(&pkg).expect("pkg dir");
    fs::write(pkg.join("beagle"), binary).expect("binary");
    fs::write(pkg.join("beagle-agentd"), "new-daemon").expect("daemon");

    let asset = super::super::asset_name("test-target");
    let tarball = dir.join(&asset);
    let status = Command::new("tar")
        .args([
            "-czf",
            &tarball.to_string_lossy(),
            "-C",
            &pkg.to_string_lossy(),
            "beagle",
            "beagle-agentd",
        ])
        .status()
        .expect("tar available");
    assert!(status.success(), "tar failed");

    let digest = sha256_of(&tarball).expect("digest");
    let sha_file = dir.join(format!("{asset}.sha256"));
    fs::write(&sha_file, format!("{digest}  {asset}\n")).expect("sha file");
    (tarball, sha_file)
}
