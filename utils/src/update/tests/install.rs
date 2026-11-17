//! Tests for the download → verify → swap pipeline (`update::install`).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use std::path::PathBuf;

use super::*;

const COMPANIONS: &[&str] = &["tool-daemon"];

fn file_url(path: &Path) -> String {
    format!("file://{}", path.display())
}

/// End-to-end install over `file://` URLs: build a fake release tarball
/// with a real checksum, install it over a fake current binary, and check
/// the swap happened with no staging file left behind.
#[test]
fn install_verifies_checksum_extracts_and_swaps_the_binary() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (tarball, sha_file) = fake_release(tmp.path(), b"#!/bin/sh\necho new\n");

    let exe = tmp.path().join("bin").join("tool");
    fs::create_dir_all(exe.parent().unwrap()).expect("bin dir");
    fs::write(&exe, "old-binary").expect("old binary");

    install(
        &file_url(&tarball),
        &file_url(&sha_file),
        &exe,
        "tool",
        COMPANIONS,
    )
    .expect("install succeeds");

    assert_eq!(fs::read(&exe).expect("read"), b"#!/bin/sh\necho new\n");
    assert!(
        !exe.parent().unwrap().join(".tool.update").exists(),
        "no staging file left behind"
    );
}

#[test]
fn sha_lines_parse_and_reject_non_digests() {
    let line = format!("{}  tool-x.tar.gz\n", "ab".repeat(32));
    assert_eq!(parse_sha(&line), Some("ab".repeat(32)));
    assert_eq!(
        parse_sha("ABCD".repeat(16).as_str()).as_deref(),
        Some("abcd".repeat(16).as_str())
    );
    assert_eq!(parse_sha("not-a-digest file"), None);
    assert_eq!(parse_sha(""), None);
}

#[test]
fn install_refuses_a_tampered_tarball() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (tarball, sha_file) = fake_release(tmp.path(), b"legit");
    // Corrupt the tarball after the checksum was published.
    fs::write(&tarball, "tampered bytes").expect("tamper");

    let exe = tmp.path().join("tool");
    fs::write(&exe, "old-binary").expect("old binary");

    let err = install(
        &file_url(&tarball),
        &file_url(&sha_file),
        &exe,
        "tool",
        COMPANIONS,
    )
    .expect_err("checksum mismatch must fail the install");
    assert!(matches!(err, Error::ChecksumMismatch { .. }), "{err}");
    assert_eq!(fs::read(&exe).expect("read"), b"old-binary", "untouched");
}

#[test]
fn install_fails_when_the_tarball_lacks_the_binary() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (tarball, sha_file) = fake_release(tmp.path(), b"x");
    let exe = tmp.path().join("other");
    fs::write(&exe, "old").expect("old binary");

    let err = install(
        &file_url(&tarball),
        &file_url(&sha_file),
        &exe,
        "other",
        &[],
    )
    .expect_err("no `other` in the tarball");
    assert!(err.to_string().contains("no `other` binary"), "{err}");
    assert_eq!(fs::read(&exe).expect("read"), b"old");
}

#[test]
fn install_also_swaps_an_installed_companion() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (tarball, sha_file) = fake_release(tmp.path(), b"new-cli");
    let bin = tmp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin dir");
    fs::write(bin.join("tool"), "old-cli").expect("old cli");
    fs::write(bin.join("tool-daemon"), "old-daemon").expect("old daemon");

    install(
        &file_url(&tarball),
        &file_url(&sha_file),
        &bin.join("tool"),
        "tool",
        COMPANIONS,
    )
    .expect("install succeeds");

    assert_eq!(fs::read(bin.join("tool")).expect("cli"), b"new-cli");
    assert_eq!(
        fs::read(bin.join("tool-daemon")).expect("daemon"),
        b"new-daemon"
    );
}

#[test]
fn install_leaves_a_companion_alone_when_it_is_not_installed() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (tarball, sha_file) = fake_release(tmp.path(), b"new-cli");
    let bin = tmp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin dir");
    fs::write(bin.join("tool"), "old-cli").expect("old cli");

    install(
        &file_url(&tarball),
        &file_url(&sha_file),
        &bin.join("tool"),
        "tool",
        COMPANIONS,
    )
    .expect("install succeeds");
    assert!(!bin.join("tool-daemon").exists(), "not created");
}

/// Builds a tarball whose root holds `tool` and `tool-daemon`, with a
/// matching `.sha256` — the release contract layout.
fn fake_release(dir: &Path, binary: &[u8]) -> (PathBuf, PathBuf) {
    let pkg = dir.join("pkg");
    fs::create_dir_all(&pkg).expect("pkg dir");
    fs::write(pkg.join("tool"), binary).expect("binary");
    fs::write(pkg.join("tool-daemon"), "new-daemon").expect("daemon");

    let asset = "tool-test-target.tar.gz";
    let tarball = dir.join(asset);
    let status = Command::new("tar")
        .args([
            "-czf",
            &tarball.to_string_lossy(),
            "-C",
            &pkg.to_string_lossy(),
            "tool",
            "tool-daemon",
        ])
        .status()
        .expect("tar available");
    assert!(status.success(), "tar failed");

    let digest = sha256_of(&tarball).expect("digest");
    let sha_file = dir.join(format!("{asset}.sha256"));
    fs::write(&sha_file, format!("{digest}  {asset}\n")).expect("sha file");
    (tarball, sha_file)
}
