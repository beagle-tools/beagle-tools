//! Tests for `beagle doctor`'s checks, against tempdir homes and PATHs.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use std::fs;
use std::os::unix::fs::PermissionsExt as _;

use super::*;
use crate::install::swap::swap_current;
use crate::test_fixture::{manifest, v, FAKE};

fn exe(path: &Path) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, "#!/bin/sh\n").unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn parses_launchd_and_systemd_programs() {
    let plist = "<dict><key>Label</key><string>com.agentd.daemon</string>\
        <key>ProgramArguments</key><array><string>/opt/bin/agentd</string><string>run</string></array></dict>";
    assert_eq!(plist_program(plist).as_deref(), Some("/opt/bin/agentd"));
    assert_eq!(plist_program("<dict></dict>"), None);
    let unit = "[Service]\nExecStart=-/home/u/.local/bin/beagle-agentd --serve\n";
    assert_eq!(
        unit_exec(unit).as_deref(),
        Some("/home/u/.local/bin/beagle-agentd")
    );
    assert_eq!(unit_exec("[Service]\n"), None);
}

#[test]
fn healthy_install_is_all_ok_and_problems_are_flagged() {
    let tmp = tempfile::tempdir().unwrap();
    let layout = Layout::new(tmp.path().join("data"));
    let reg = [FAKE];

    // A healthy fake 1.0.0 install.
    let dir = layout.version_dir("fake", v("1.0.0"));
    exe(&dir.join("bin/beagle-fake"));
    fs::write(dir.join("extension.toml"), manifest("1.0.0")).unwrap();
    swap_current(&layout, "fake", v("1.0.0")).unwrap();

    // This binary on PATH, plus a legacy agentctl.
    let bin = tmp.path().join("bin");
    let me = bin.join("beagle");
    exe(&me);
    exe(&bin.join("agentctl"));

    // A launchd unit pointing at a deleted binary.
    let home = tmp.path().join("home");
    let agents = home.join("Library/LaunchAgents");
    fs::create_dir_all(&agents).unwrap();
    fs::write(
        agents.join("com.beagle.agentd.plist"),
        "<key>ProgramArguments</key><array><string>/gone/beagle-agentd</string></array>",
    )
    .unwrap();

    let checks = run(&Inputs {
        layout: &layout,
        registry: &reg,
        path_var: bin.as_os_str(),
        exe: &me,
        home: Some(&home),
    });
    let has = |status: Status, needle: &str| {
        checks
            .iter()
            .any(|c| c.status == status && c.message.contains(needle))
    };
    assert!(has(Status::Ok, "is writable"), "{checks:#?}");
    assert!(has(Status::Ok, "fake 1.0.0"), "{checks:#?}");
    assert!(
        has(Status::Ok, "beagle on PATH is this binary"),
        "{checks:#?}"
    );
    assert!(has(Status::Warn, "agentctl"), "{checks:#?}");
    assert!(
        has(Status::Fail, "/gone/beagle-agentd, which does not exist"),
        "{checks:#?}"
    );
}

#[test]
fn a_dangling_current_link_fails() {
    let tmp = tempfile::tempdir().unwrap();
    let layout = Layout::new(tmp.path());
    fs::create_dir_all(layout.ext_dir("fake")).unwrap();
    std::os::unix::fs::symlink("9.9.9", layout.current_link("fake")).unwrap();
    let reg = [FAKE];
    let checks = run(&Inputs {
        layout: &layout,
        registry: &reg,
        path_var: OsStr::new(""),
        exe: Path::new("/nowhere/beagle"),
        home: None,
    });
    assert!(
        checks
            .iter()
            .any(|c| c.status == Status::Fail && c.message.contains("does not resolve")),
        "{checks:#?}"
    );
    assert!(
        checks
            .iter()
            .any(|c| c.status == Status::Warn && c.message.contains("not on PATH")),
        "{checks:#?}"
    );
}
