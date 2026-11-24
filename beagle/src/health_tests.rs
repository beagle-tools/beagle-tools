//! Tests for `beagle doctor`'s setup checks, over a temp HOME and a PATH of
//! fake programs (never the real HOME or PATH).
#![allow(clippy::expect_used, clippy::unwrap_used)] // panicking is the correct failure mode in tests

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::PathBuf;

use super::*;

/// A shell script `name` in `dir` that prints `out` and exits `code`.
fn program(dir: &Path, name: &str, out: &str, code: i32) -> PathBuf {
    fs::create_dir_all(dir).unwrap();
    let path = dir.join(name);
    fs::write(&path, format!("#!/bin/sh\necho '{out}'\nexit {code}\n")).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    path
}

#[test]
fn an_empty_machine_warns_about_everything_with_the_fix() {
    let home = tempfile::tempdir().unwrap();
    let empty = home.path().join("empty-bin");
    fs::create_dir_all(&empty).unwrap();
    let checks = checks(home.path(), empty.as_os_str());
    assert_eq!(checks.len(), 5);
    for check in &checks {
        // No agent services installed is fine; everything else is missing.
        let services = matches!(check, Check::Ok(s) if s.starts_with("agent services"));
        assert!(services || matches!(check, Check::Warn(_)), "{check:?}");
    }
    let text: Vec<String> = checks.iter().map(Check::line).collect();
    let text = text.join("\n");
    assert!(text.contains("gh is not installed"));
    assert!(text.contains("tmux is not installed"));
    assert!(text.contains("run `beagle setup`"));
}

#[test]
fn installed_and_signed_in_tools_pass() {
    let home = tempfile::tempdir().unwrap();
    let bin = home.path().join("bin");
    program(&bin, "gh", "Logged in", 0);
    program(&bin, "tmux", "", 0);
    let path = bin.as_os_str();
    assert_eq!(gh(path), Check::Ok("gh is signed in".to_owned()));
    assert_eq!(tmux(path), Check::Ok("tmux is installed".to_owned()));
}

#[test]
fn gh_signed_out_is_a_warning() {
    let home = tempfile::tempdir().unwrap();
    let bin = home.path().join("bin");
    program(&bin, "gh", "not logged in", 1);
    assert_eq!(
        gh(bin.as_os_str()),
        Check::Warn("gh is not signed in — run `gh auth login`".to_owned())
    );
}

#[test]
fn hooks_and_skills_pass_once_installed() {
    let home = tempfile::tempdir().unwrap();
    for agent in beagle_rca::skill::Agent::ALL {
        for skill in beagle_rca::skill::Skill::ALL {
            beagle_rca::skill::install(skill, agent, home.path()).unwrap();
        }
    }
    for agent in beagle_scout::skill::Agent::ALL {
        for skill in beagle_scout::skill::Skill::ALL {
            agent.install(skill, home.path()).unwrap();
        }
    }
    assert_eq!(
        skills(home.path()),
        Check::Ok("agent skills are current".to_owned())
    );
    let path = beagle_scout::hooks::settings_path(home.path());
    let mut settings = beagle_scout::hooks::load(&path).unwrap();
    beagle_scout::hooks::install(&mut settings, "/usr/local/bin/beagle scout hook");
    beagle_scout::hooks::save(&path, &settings).unwrap();
    assert!(matches!(scout_hooks(home.path()), Check::Ok(_)));
}

#[test]
fn services_still_running_a_retired_binary_warn_with_the_fix() {
    let home = tempfile::tempdir().unwrap();
    let agents = home.path().join("Library/LaunchAgents");
    fs::create_dir_all(&agents).unwrap();
    fs::write(
        agents.join("com.agentd.daemon.plist"),
        "<array>\n<string>/opt/homebrew/bin/agentd</string>\n</array>",
    )
    .unwrap();
    fs::write(
        agents.join("com.beagle.agentd.plist"),
        "<array>\n<string>/opt/homebrew/bin/beagle</string>\n<string>rca</string>\n\
         <string>agentd</string>\n<string>run</string>\n</array>",
    )
    .unwrap();
    let check = agent_services(home.path());
    assert!(matches!(check, Check::Warn(_)), "{check:?}");
    let text = check.line();
    assert!(
        text.contains("`agentd` (run `beagle agent daemon install`)"),
        "{text}"
    );
    assert!(!text.contains("beagle-agentd"), "{text}");
}

#[test]
fn a_systemd_unit_running_beagle_agentd_warns() {
    let home = tempfile::tempdir().unwrap();
    let dir = home.path().join(".config/systemd/user");
    fs::create_dir_all(&dir).unwrap();
    let unit = dir.join("beagle-agentd.service");
    fs::write(&unit, "[Service]\nExecStart=/usr/local/bin/beagle-agentd\n").unwrap();
    assert!(
        matches!(agent_services(home.path()), Check::Warn(t) if t.contains("beagle rca agent install"))
    );
    fs::write(
        &unit,
        "[Service]\nExecStart=/usr/local/bin/beagle rca agentd run\n",
    )
    .unwrap();
    assert!(matches!(agent_services(home.path()), Check::Ok(_)));
}
