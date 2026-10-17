// Tests fail loudly on setup errors; unwrap is the clearest way to do that.
#![allow(clippy::unwrap_used)]

use super::*;

#[test]
fn targets_are_exact_matches() {
    assert_eq!(exact_target("agentd-abc"), "=agentd-abc");
}

#[test]
fn command_resolves_with_augmented_path() {
    let cmd = command();
    let path = cmd
        .get_envs()
        .find(|(k, _)| *k == std::ffi::OsStr::new("PATH"))
        .and_then(|(_, v)| v);
    assert!(path.is_some_and(|p| p.to_string_lossy().contains("/opt/homebrew/bin")));
}

/// Regression: killing `name` must not prefix-match and kill `name-long`.
/// Skipped where tmux is not installed (e.g. a bare CI runner).
#[test]
fn kill_does_not_take_down_a_longer_named_session() {
    if beagle_utils::process::which("tmux").is_none() {
        return;
    }
    let short = format!("agentcore-test-{}", std::process::id());
    let long = format!("{short}-long");
    let started = command()
        .args(["new-session", "-d", "-s", &long, "sleep 60"])
        .status()
        .unwrap();
    assert!(started.success());

    // `short` does not exist: plain `-t short` would have matched `long`.
    kill_session(&short).unwrap();
    let survived = has_session(&long).unwrap();
    kill_session(&long).unwrap();

    assert!(survived, "kill_session({short}) killed {long}");
    assert!(!has_session(&long).unwrap());
}
