//! Tests for dispatch resolution and the env contract, with a fake `PATH`.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::time::Duration;

use super::*;
use crate::install::{install, Request};
use crate::test_fixture::{v, Server, FAKE, TARGET};

const REG: &[ExtensionSpec] = &[FAKE];

fn exe(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, "#!/bin/sh\n").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    path
}

#[test]
fn reserved_names_never_dispatch() {
    let tmp = tempfile::tempdir().unwrap();
    let layout = Layout::new(tmp.path());
    let err = resolve("doctor", REG, &layout, OsStr::new("")).unwrap_err();
    assert!(matches!(err, Error::Reserved { .. }), "{err}");
}

#[test]
fn registry_extension_not_installed_gets_the_install_hint() {
    let tmp = tempfile::tempdir().unwrap();
    let bin = tmp.path().join("bin");
    fs::create_dir_all(&bin).unwrap();
    // Even with a same-named PATH binary, the registry wins.
    exe(&bin, "beagle-fake");
    let err = resolve("fake", REG, &Layout::new(tmp.path()), bin.as_os_str()).unwrap_err();
    assert_eq!(
        err.to_string(),
        "\"fake\" is not installed — run: beagle extensions install fake"
    );
    assert_eq!(err.exit_code(), 1);
}

#[test]
fn installed_extension_resolves_to_its_versioned_binary() {
    let tmp = tempfile::tempdir().unwrap();
    let layout = Layout::new(tmp.path().join("data"));
    let mut server = Server::new(tmp.path());
    let source = server.source();
    let release = server.publish("1.2.3");
    install(&Request {
        spec: &FAKE,
        release: &release,
        source: &source,
        target: TARGET,
        layout: &layout,
        base_version: v("0.1.0"),
        lock_wait: Duration::ZERO,
    })
    .unwrap();

    let resolved = resolve("fake", REG, &layout, OsStr::new("")).unwrap();
    let Resolved::Installed {
        version, dir, bin, ..
    } = &resolved
    else {
        panic!("{resolved:?}");
    };
    assert_eq!(*version, v("1.2.3"));
    assert_eq!(*dir, layout.version_dir("fake", v("1.2.3")));
    assert_eq!(*bin, layout.current_link("fake").join("bin/beagle-fake"));

    let cmd = command(
        &resolved,
        &["a".into()],
        Path::new("/usr/bin/beagle"),
        "fake",
    );
    let envs: Vec<(String, Option<String>)> = cmd
        .get_envs()
        .map(|(k, v)| {
            (
                k.to_string_lossy().into(),
                v.map(|v| v.to_string_lossy().into()),
            )
        })
        .collect();
    let get = |key: &str| {
        envs.iter()
            .find(|(k, _)| k == key)
            .and_then(|(_, v)| v.clone())
    };
    assert_eq!(get("BEAGLE_BASE").as_deref(), Some("/usr/bin/beagle"));
    assert_eq!(
        get("BEAGLE_BASE_VERSION"),
        Some(Version::current().to_string())
    );
    assert_eq!(get("BEAGLE_EXTENSION").as_deref(), Some("fake"));
    assert_eq!(get("BEAGLE_EXTENSION_VERSION").as_deref(), Some("1.2.3"));
    assert_eq!(
        get("BEAGLE_EXTENSION_DIR"),
        Some(dir.to_string_lossy().into_owned())
    );
    assert_eq!(get("BEAGLE_INVOKED_AS").as_deref(), Some("beagle fake"));
    assert_eq!(cmd.get_args().collect::<Vec<_>>(), ["a"]);
}

#[test]
fn unregistered_names_fall_back_to_path() {
    let tmp = tempfile::tempdir().unwrap();
    let first = tmp.path().join("first");
    let second = tmp.path().join("second");
    fs::create_dir_all(&first).unwrap();
    fs::create_dir_all(&second).unwrap();
    let want = exe(&first, "beagle-devtool");
    exe(&second, "beagle-devtool");
    let path = std::env::join_paths([&first, &second]).unwrap();

    let resolved = resolve("devtool", REG, &Layout::new(tmp.path()), &path).unwrap();
    assert_eq!(
        resolved,
        Resolved::OnPath {
            name: "devtool".into(),
            bin: want
        }
    );
    let cmd = command(&resolved, &[], Path::new("/b"), "devtool");
    let removed: Vec<_> = cmd
        .get_envs()
        .filter(|(_, v)| v.is_none())
        .map(|(k, _)| k.to_owned())
        .collect();
    assert!(removed.iter().any(|k| k == "BEAGLE_EXTENSION_VERSION"));
}

#[test]
fn unknown_names_exit_2_with_a_suggestion() {
    let tmp = tempfile::tempdir().unwrap();
    let err = resolve("fkae", REG, &Layout::new(tmp.path()), OsStr::new("")).unwrap_err();
    assert_eq!(
        err.to_string(),
        "unknown command \"fkae\" — did you mean \"fake\"?"
    );
    assert_eq!(err.exit_code(), 2);
    let err = resolve("nope", REG, &Layout::new(tmp.path()), OsStr::new("")).unwrap_err();
    assert_eq!(err.to_string(), "unknown command \"nope\"");
}

#[test]
fn suggestions_cover_base_commands() {
    assert_eq!(did_you_mean("doctr", REG), " — did you mean \"doctor\"?");
    assert_eq!(did_you_mean("zzzzzzzz", REG), "");
}
