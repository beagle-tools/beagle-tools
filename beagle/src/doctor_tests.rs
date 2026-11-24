//! Tests for `beagle doctor`'s PATH scan, over temp directories.
#![allow(clippy::expect_used)] // panicking is the correct failure mode in tests

use std::ffi::OsString;
use std::fs;

use super::*;

fn executable(dir: &Path, name: &str) -> PathBuf {
    fs::create_dir_all(dir).expect("dir");
    let path = dir.join(name);
    fs::write(&path, "#!/bin/sh\n").expect("write");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("chmod");
    }
    path
}

fn path_of(dirs: &[&Path]) -> OsString {
    std::env::join_paths(dirs).expect("joinable")
}

#[test]
fn a_clean_path_has_nothing_to_report() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let bin = tmp.path().join("bin");
    let this = resolve(&executable(&bin, "beagle"));
    executable(&bin, "unrelated");
    assert_eq!(scan(&path_of(&[&bin]), Some(&this)), []);
}

#[test]
fn finds_retired_tools_and_other_beagles_in_path_order() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (early, ours, late) = (
        tmp.path().join("early"),
        tmp.path().join("ours"),
        tmp.path().join("late"),
    );
    let shadow = executable(&early, "beagle");
    let this = resolve(&executable(&ours, "beagle"));
    let scout = executable(&ours, "scout");
    let duplicate = executable(&late, "beagle");

    let findings = scan(&path_of(&[&early, &ours, &late]), Some(&this));
    assert_eq!(
        findings,
        [
            Finding::Shadowing(shadow),
            Finding::Retired {
                name: "scout",
                path: scout
            },
            Finding::Duplicate(duplicate),
        ]
    );
}

#[test]
fn a_directory_listed_twice_is_reported_once() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let bin = tmp.path().join("bin");
    let recap = executable(&bin, "recap");
    let findings = scan(&path_of(&[&bin, &bin]), None);
    assert_eq!(
        findings,
        [Finding::Retired {
            name: "recap",
            path: recap
        }]
    );
}

#[cfg(unix)]
#[test]
fn non_executables_are_ignored() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let bin = tmp.path().join("bin");
    fs::create_dir_all(&bin).expect("dir");
    fs::write(bin.join("standup"), "notes").expect("write");
    assert_eq!(scan(&path_of(&[&bin]), None), []);
}

#[test]
fn the_serve_service_is_reported_and_only_a_down_one_fails() {
    use beagle_serve::service::Status;
    let (line, ok) = serve_line(&Status::NotInstalled);
    assert!(ok, "not installing it is a choice");
    assert!(line.contains("beagle serve install"), "{line}");
    let running = Status::Loaded {
        plist: PathBuf::from("/p"),
        state: "running".into(),
        pid: Some(9),
    };
    assert_eq!(
        serve_line(&running),
        ("✓ beagle serve: running (pid 9)".into(), true)
    );
    let down = Status::Loaded {
        plist: PathBuf::from("/p"),
        state: "waiting".into(),
        pid: None,
    };
    assert!(!serve_line(&down).1);
    assert!(!serve_line(&Status::NotLoaded(PathBuf::from("/p"))).1);
}

#[test]
fn the_agentd_stack_is_retired_too() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let bin = tmp.path().join("bin");
    let names = ["agentctl", "agenttop", "agentd"];
    let expected: Vec<Finding> = names
        .iter()
        .map(|&name| Finding::Retired {
            name,
            path: executable(&bin, name),
        })
        .collect();
    assert_eq!(scan(&path_of(&[&bin]), None), expected);
}

#[test]
fn beagle_agentd_is_not_flagged_while_beagle_still_ships_it() {
    // It stays in the beagle formula for one release so services installed
    // before `beagle rca agentd run` keep working; health.rs warns instead.
    let tmp = tempfile::tempdir().expect("tempdir");
    let bin = tmp.path().join("bin");
    executable(&bin, "beagle-agentd");
    assert_eq!(scan(&path_of(&[&bin]), None), []);
}

#[test]
fn each_retired_binary_names_the_command_that_replaced_it() {
    assert_eq!(replacement("agentctl"), "beagle agent");
    assert_eq!(replacement("agenttop"), "beagle agent top");
    assert_eq!(replacement("agentd"), "beagle agent daemon run");
    assert_eq!(replacement("scout"), "beagle scout");
    let line = describe(&Finding::Retired {
        name: "agentctl",
        path: PathBuf::from("/usr/local/bin/agentctl"),
    });
    assert!(line.contains("it is `beagle agent` now"), "{line}");
}
