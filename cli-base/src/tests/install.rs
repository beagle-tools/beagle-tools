//! Tests for the install pipeline over a `file://` fake release server.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use std::time::{Duration, UNIX_EPOCH};

use super::*;
use crate::test_fixture::{manifest, v, Server, FAKE, TARGET};

struct Env {
    _tmp: tempfile::TempDir,
    layout: Layout,
    server: Server,
    source: ReleaseSource,
}

fn env() -> Env {
    let tmp = tempfile::tempdir().unwrap();
    let layout = Layout::new(tmp.path().join("data/beagle"));
    let server = Server::new(tmp.path());
    let source = server.source();
    Env {
        _tmp: tmp,
        layout,
        server,
        source,
    }
}

fn request<'a>(env: &'a Env, release: &'a Release) -> Request<'a> {
    Request {
        spec: &FAKE,
        release,
        source: &env.source,
        target: TARGET,
        layout: &env.layout,
        base_version: v("0.1.0"),
        lock_wait: Duration::ZERO,
    }
}

#[test]
fn happy_path_lays_out_the_version_and_points_current_at_it() {
    let mut env = env();
    let release = env.server.publish("0.29.0");
    let done = install(&request(&env, &release)).unwrap();
    assert_eq!(done.version, v("0.29.0"));
    assert!(!done.was_present);

    let dir = env.layout.version_dir("fake", v("0.29.0"));
    assert!(is_executable_file(&dir.join("bin/beagle-fake")));
    assert!(dir.join(MANIFEST).is_file());
    let record: toml::Table = fs::read_to_string(dir.join(INSTALL_RECORD))
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(record["tag"].as_str(), Some("beagle-fake-v0.29.0"));
    assert_eq!(record["sha256"].as_str().map(str::len), Some(64));
    assert_eq!(
        fs::read_link(env.layout.current_link("fake")).unwrap(),
        Path::new("0.29.0"),
        "relative link"
    );
    assert_eq!(env.layout.current_version("fake"), Some(v("0.29.0")));
}

#[test]
fn reinstalling_a_present_version_only_moves_current() {
    let mut env = env();
    let old = env.server.publish("0.28.0");
    let new = env.server.publish("0.29.0");
    install(&request(&env, &old)).unwrap();
    install(&request(&env, &new)).unwrap();
    let again = install(&request(&env, &old)).unwrap();
    assert!(again.was_present);
    assert_eq!(env.layout.current_version("fake"), Some(v("0.28.0")));
}

#[test]
fn checksum_mismatch_aborts_and_keeps_current() {
    let mut env = env();
    let good = env.server.publish("0.28.0");
    install(&request(&env, &good)).unwrap();
    let bad = env.server.publish("0.29.0");
    fs::write(env.server.tarball("0.29.0"), b"tampered").unwrap();

    let err = install(&request(&env, &bad)).unwrap_err();
    assert!(
        err.to_string()
            .starts_with("checksum mismatch for beagle-fake-ext-test-target.tar.gz"),
        "{err}"
    );
    assert_eq!(env.layout.current_version("fake"), Some(v("0.28.0")));
    assert!(!env.layout.version_dir("fake", v("0.29.0")).exists());
    assert_no_staging(&env);
}

#[test]
fn truncated_tarball_fails_cleanly() {
    let mut env = env();
    let release = env.server.publish("0.29.0");
    let tarball = env.server.tarball("0.29.0");
    let bytes = fs::read(&tarball).unwrap();
    fs::write(&tarball, &bytes[..bytes.len() / 2]).unwrap();
    // Re-publish the digest of the truncated bytes so extraction is reached.
    let digest = sha256_of(&tarball).unwrap();
    let sha = tarball.with_file_name(format!("{}.sha256", FAKE.asset_name(TARGET)));
    fs::write(sha, format!("{digest}  x\n")).unwrap();

    let err = install(&request(&env, &release)).unwrap_err();
    assert!(matches!(err, Error::Tool { tool: "tar", .. }), "{err}");
    assert_eq!(env.layout.current_version("fake"), None);
    assert_no_staging(&env);
}

#[test]
fn tarball_missing_the_declared_bin_is_rejected() {
    let mut env = env();
    let text = manifest("0.29.0");
    let release = env
        .server
        .publish_raw("0.29.0", &[("extension.toml", text.as_bytes(), false)]);
    let err = install(&request(&env, &release)).unwrap_err();
    assert!(err.to_string().contains("beagle-fake"), "{err}");
    assert_eq!(env.layout.current_version("fake"), None);
}

#[test]
fn manifest_for_another_extension_is_rejected() {
    let mut env = env();
    let text = manifest("0.29.0").replace("name = \"fake\"", "name = \"agentd\"");
    let release = env.server.publish_raw(
        "0.29.0",
        &[
            ("extension.toml", text.as_bytes(), false),
            ("beagle-fake", b"#!/bin/sh\n", true),
        ],
    );
    let err = install(&request(&env, &release)).unwrap_err();
    assert!(matches!(err, Error::NameMismatch { .. }), "{err}");
}

#[test]
fn stale_staging_from_a_killed_install_is_cleaned_and_current_survives() {
    let mut env = env();
    let good = env.server.publish("0.28.0");
    install(&request(&env, &good)).unwrap();
    // A process killed mid-download leaves its staging dir behind.
    let stale = env.layout.staging_dir("fake", 999_999);
    fs::create_dir_all(stale.join("unpacked")).unwrap();
    fs::write(stale.join("partial.tar.gz"), b"half").unwrap();
    assert_eq!(env.layout.current_version("fake"), Some(v("0.28.0")));

    let next = env.server.publish("0.29.0");
    install(&request(&env, &next)).unwrap();
    assert!(!stale.exists(), "stale staging removed");
    assert_eq!(env.layout.current_version("fake"), Some(v("0.29.0")));
}

#[test]
fn a_second_installer_fails_while_the_lock_is_held() {
    let mut env = env();
    let release = env.server.publish("0.29.0");
    let _held = lock::acquire(&env.layout.lock_file("fake"), "fake", Duration::ZERO).unwrap();
    let err = install(&request(&env, &release)).unwrap_err();
    assert_eq!(
        err.to_string(),
        "another beagle is installing fake — try again when it finishes"
    );
}

#[test]
fn prune_keeps_current_and_the_newest_others() {
    let mut env = env();
    for version in ["0.1.0", "0.2.0", "0.3.0", "0.4.0"] {
        let release = env.server.publish(version);
        install(&request(&env, &release)).unwrap();
    }
    swap::use_version(&env.layout, "fake", v("0.1.0")).unwrap();
    let removed = swap::prune(&env.layout, "fake", 1).unwrap();
    assert_eq!(removed, [v("0.3.0"), v("0.2.0")]);
    assert_eq!(
        env.layout.installed_versions("fake"),
        [v("0.4.0"), v("0.1.0")]
    );
    assert_eq!(env.layout.current_version("fake"), Some(v("0.1.0")));
    assert_eq!(swap::prune(&env.layout, "fake", 0).unwrap(), [v("0.4.0")]);
}

#[test]
fn use_rejects_a_version_that_is_not_installed() {
    let mut env = env();
    let release = env.server.publish("0.29.0");
    install(&request(&env, &release)).unwrap();
    let err = swap::use_version(&env.layout, "fake", v("0.1.0")).unwrap_err();
    assert!(matches!(err, Error::VersionNotInstalled { .. }), "{err}");
    assert_eq!(env.layout.current_version("fake"), Some(v("0.29.0")));
}

#[test]
fn uninstall_removes_current_then_everything() {
    let mut env = env();
    for version in ["0.1.0", "0.2.0"] {
        let release = env.server.publish(version);
        install(&request(&env, &release)).unwrap();
    }
    assert_eq!(
        swap::uninstall(&env.layout, "fake", false).unwrap(),
        [v("0.1.0")]
    );
    assert_eq!(env.layout.current_version("fake"), None);
    assert_eq!(swap::uninstall(&env.layout, "fake", true).unwrap(), []);
    assert!(!env.layout.ext_dir("fake").exists());
    assert!(matches!(
        swap::uninstall(&env.layout, "fake", true),
        Err(Error::NotInstalled { .. })
    ));
}

#[test]
fn rfc3339_formats_utc() {
    assert_eq!(rfc3339(UNIX_EPOCH), "1970-01-01T00:00:00Z");
    let t = UNIX_EPOCH + Duration::from_secs(1_792_150_245);
    assert_eq!(rfc3339(t), "2026-10-16T11:30:45Z");
    let leap = UNIX_EPOCH + Duration::from_secs(951_782_400); // 2000-02-29
    assert_eq!(rfc3339(leap), "2000-02-29T00:00:00Z");
}

#[test]
fn sha_lines_parse_and_reject_non_digests() {
    assert_eq!(
        parse_sha(&format!("{}  x\n", "AB".repeat(32))),
        Some("ab".repeat(32))
    );
    assert_eq!(parse_sha("nope x"), None);
    assert_eq!(parse_sha(""), None);
}

fn assert_no_staging(env: &Env) {
    let leftovers: Vec<_> = fs::read_dir(env.layout.ext_dir("fake"))
        .map(|d| {
            d.filter_map(std::result::Result::ok)
                .map(|e| e.file_name())
                .collect()
        })
        .unwrap_or_default();
    assert!(
        !leftovers.iter().any(|n| n
            .to_string_lossy()
            .starts_with(crate::paths::STAGING_PREFIX)),
        "staging left behind: {leftovers:?}"
    );
}
