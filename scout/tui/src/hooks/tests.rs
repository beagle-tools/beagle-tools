//! Settings merge tests: idempotence, key order, upgrades and removal.

// Tests fail loudly on setup errors; unwrap/expect/panic is the clearest way.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;

const CMD: &str = "/Users/me/.cargo/bin/beagle scout hook";
/// What a standalone `scout` install wrote before scout moved into beagle.
const LEGACY: &str = "/Users/me/.cargo/bin/scout hook";

#[test]
fn standalone_installs_are_upgraded_to_beagle() {
    let mut settings = json!({});
    install(&mut settings, LEGACY);
    assert!(install(&mut settings, CMD));
    assert_eq!(settings["hooks"]["PreToolUse"].as_array().unwrap().len(), 1);
    assert!(status(&settings)
        .iter()
        .all(|(_, c)| c.as_deref() == Some(CMD)));
    assert!(uninstall(&mut settings));
    assert_eq!(settings, json!({}));
}

fn existing() -> Value {
    serde_json::from_str(
        r#"{
  "model": "opus",
  "hooks": {
    "PreToolUse": [{"matcher": "Edit", "hooks": [{"type": "command", "command": "fmt.sh"}]}],
    "Stop": [{"hooks": [{"type": "command", "command": "notify.sh"}]}]
  },
  "permissions": {"allow": []}
}"#,
    )
    .unwrap()
}

#[test]
fn install_is_idempotent_and_uninstall_restores() {
    let original = existing();
    let mut settings = original.clone();
    assert!(install(&mut settings, CMD));
    assert!(!install(&mut settings, CMD));
    assert_eq!(settings["hooks"]["PreToolUse"].as_array().unwrap().len(), 2);
    assert!(status(&settings)
        .iter()
        .all(|(_, c)| c.as_deref() == Some(CMD)));
    assert!(uninstall(&mut settings));
    assert_eq!(settings, original);
    assert_eq!(
        serde_json::to_string(&settings).unwrap(),
        serde_json::to_string(&original).unwrap(),
        "key order must survive"
    );
    assert!(!uninstall(&mut settings));
}

#[test]
fn install_into_empty_and_remove_cleanly() {
    let mut settings = json!({});
    assert!(install(&mut settings, CMD));
    assert_eq!(settings["hooks"]["PreToolUse"][0]["matcher"], "Bash");
    assert!(settings["hooks"]["PreToolUse"][0]["hooks"][0]
        .get("async")
        .is_none());
    assert_eq!(settings["hooks"]["PostToolUseFailure"][0]["matcher"], "*");
    assert_eq!(
        settings["hooks"]["PostToolUse"][0]["hooks"][0]["async"],
        true
    );
    assert!(settings["hooks"]["Stop"][0].get("matcher").is_none());
    assert_eq!(settings["hooks"]["Stop"][0]["hooks"][0]["async"], true);
    assert!(uninstall(&mut settings));
    assert_eq!(settings, json!({}));
}

#[test]
fn old_installs_are_upgraded() {
    let mut settings = json!({"hooks": {"PostToolUse": [
        {"matcher": "Bash", "hooks": [{"type": "command", "command": CMD, "timeout": 10}]}
    ]}});
    assert!(install(&mut settings, CMD));
    assert_eq!(
        settings["hooks"]["PostToolUse"].as_array().unwrap().len(),
        1
    );
    assert_eq!(settings["hooks"]["PostToolUse"][0]["matcher"], "*");
    assert_eq!(
        settings["hooks"]["PostToolUse"][0]["hooks"][0]["async"],
        true
    );
    assert!(!install(&mut settings, CMD), "now up to date");
}

#[test]
fn moved_binary_updates_the_command() {
    let mut settings = json!({});
    install(&mut settings, CMD);
    assert!(install(&mut settings, "/opt/beagle scout hook"));
    assert_eq!(
        status(&settings)[0].1.as_deref(),
        Some("/opt/beagle scout hook")
    );
    assert_eq!(settings["hooks"]["PreToolUse"].as_array().unwrap().len(), 1);
}

#[test]
fn recognises_only_scout_commands() {
    assert!(is_ours("beagle scout hook"));
    assert!(is_ours("'/a b/beagle' scout hook"));
    assert!(is_ours(CMD));
    assert!(is_ours("scout hook"));
    assert!(is_ours("'/a b/scout' hook"));
    assert!(is_ours(LEGACY));
    assert!(!is_ours("scoutish hook"));
    assert!(!is_ours("/bin/notbeagle scout hook"));
    assert!(!is_ours("/bin/beagle hook"));
    assert!(!is_ours("/bin/scout watch"));
    assert!(!is_ours("fmt.sh"));
}

#[test]
fn quoting() {
    assert_eq!(
        hook_command(Path::new("/usr/bin/beagle")),
        "/usr/bin/beagle scout hook"
    );
    assert_eq!(
        hook_command(Path::new("/a b/beagle")),
        "'/a b/beagle' scout hook"
    );
}

#[test]
fn save_backs_up_and_load_reads_back() {
    let dir = tempfile::tempdir().unwrap();
    let path = settings_path(dir.path());
    assert_eq!(load(&path).unwrap(), json!({}));
    assert!(save(&path, &json!({"a": 1})).unwrap().is_none());
    let backup = save(&path, &json!({"a": 2})).unwrap().unwrap();
    assert_eq!(load(&backup).unwrap(), json!({"a": 1}));
    assert_eq!(load(&path).unwrap(), json!({"a": 2}));
    // The first backup is the user's pre-scout settings: never overwritten.
    save(&path, &json!({"a": 3})).unwrap();
    assert_eq!(load(&backup).unwrap(), json!({"a": 1}));
}
