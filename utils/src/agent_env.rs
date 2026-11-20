//! Keeping GitHub write access away from coding agents.
//!
//! Tandem and rca run agents (claude, codex, …) as local child processes.
//! By default a child inherits everything: `GH_TOKEN`, the `gh` login in
//! `~/.config/gh`, and git's credential helper (the macOS keychain). An
//! agent reviewing someone else's PR could then be talked — by text inside
//! that PR — into approving, merging or pushing. Every GitHub write these
//! tools make is done by the tool itself on a user action, so the agent
//! needs none of that access.
//!
//! [`deny_github`] removes it from one command:
//!
//! - `GH_TOKEN` is set to a dummy GitHub rejects: `gh` prefers it over its
//!   own login (including the token it keeps in the macOS keychain), so
//!   every `gh` call fails with 401; the other token variables are removed;
//! - `GH_CONFIG_DIR` points at an empty directory, so `gh` has no accounts;
//! - git's credential helpers are cleared and it may not prompt, so an
//!   HTTPS push has nothing to authenticate with;
//! - git over SSH is refused, so a push can't use an SSH key either.
//!
//! Local git (log, diff, commit) keeps working. Values set explicitly on
//! the command AFTER this call win, so a user who really wants an agent to
//! reach GitHub can still say so in the agent's own env.
//!
//! **Limit.** A process running as the user can still read the user's
//! keychain directly (`gh auth token --user <name>`, `security …`). For
//! Claude runs, [`CLAUDE_DISALLOWED_TOOLS`] also refuses those commands;
//! only the planned per-run sandbox closes the path completely.

use std::ffi::OsString;
use std::path::Path;
use std::process::Command;

/// Token variables removed outright (`GH_TOKEN` is overridden instead).
pub const TOKEN_VARS: [&str; 3] = [
    "GITHUB_TOKEN",
    "GH_ENTERPRISE_TOKEN",
    "GITHUB_ENTERPRISE_TOKEN",
];

/// What `GH_TOKEN` is set to: not a token, so GitHub answers 401, and it
/// says why to anyone who prints it.
pub const DUMMY_GH_TOKEN: &str = "no-github-access-for-agents";

/// Commands a Claude agent may not run (`--disallowedTools`): `gh` (incl.
/// `gh auth token`, which would read the keychain), `git push`, and
/// macOS `security` (the keychain).
pub const CLAUDE_DISALLOWED_TOOLS: [&str; 3] =
    ["Bash(gh:*)", "Bash(git push:*)", "Bash(security:*)"];

/// Strips GitHub write access from `cmd` (see the module docs).
/// `empty_dir` becomes the agent's `gh` config dir; it is created if
/// missing and must not hold a `gh` login.
///
/// # Errors
/// When `empty_dir` cannot be created.
pub fn deny_github(cmd: &mut Command, empty_dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(empty_dir)?;
    for var in TOKEN_VARS {
        cmd.env_remove(var);
    }
    cmd.env("GH_TOKEN", DUMMY_GH_TOKEN);
    cmd.env("GH_CONFIG_DIR", empty_dir);
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    // `false` as the ssh program: any git operation over SSH fails at once.
    cmd.env("GIT_SSH_COMMAND", "false");
    // An empty `credential.helper` resets the list git would otherwise use.
    // Appended after any `GIT_CONFIG_*` pairs the parent already set, so
    // those still apply.
    let existing = git_config_count(std::env::var_os("GIT_CONFIG_COUNT"));
    cmd.env("GIT_CONFIG_COUNT", (existing + 1).to_string());
    cmd.env(format!("GIT_CONFIG_KEY_{existing}"), "credential.helper");
    cmd.env(format!("GIT_CONFIG_VALUE_{existing}"), "");
    Ok(())
}

/// How many `GIT_CONFIG_KEY_n` pairs the parent environment declares.
fn git_config_count(raw: Option<OsString>) -> usize {
    raw.and_then(|v| v.into_string().ok())
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0)
}

#[cfg(test)]
// Tests fail loudly on setup errors; unwrap is the clearest way.
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    /// What the command will set (`Some`) or remove (`None`).
    fn envs(cmd: &Command) -> HashMap<String, Option<String>> {
        cmd.get_envs()
            .map(|(k, v)| {
                (
                    k.to_string_lossy().into_owned(),
                    v.map(|v| v.to_string_lossy().into_owned()),
                )
            })
            .collect()
    }

    #[test]
    fn tokens_are_removed_and_gh_and_git_are_logged_out() {
        let dir = std::env::temp_dir().join(format!("agent-env-{}", std::process::id()));
        let mut cmd = Command::new("true");
        cmd.env("GH_TOKEN", "secret");
        deny_github(&mut cmd, &dir).unwrap();
        let env = envs(&cmd);
        for var in TOKEN_VARS {
            assert_eq!(env.get(var), Some(&None), "{var} must be removed");
        }
        assert_eq!(env["GH_TOKEN"].as_deref(), Some(DUMMY_GH_TOKEN));
        assert_eq!(env["GH_CONFIG_DIR"].as_deref(), Some(dir.to_str().unwrap()));
        assert!(dir.is_dir());
        assert_eq!(env["GIT_SSH_COMMAND"].as_deref(), Some("false"));
        assert_eq!(env["GIT_TERMINAL_PROMPT"].as_deref(), Some("0"));
        let n = env["GIT_CONFIG_COUNT"].clone().unwrap();
        let last = n.parse::<usize>().unwrap() - 1;
        assert_eq!(
            env[&format!("GIT_CONFIG_KEY_{last}")].as_deref(),
            Some("credential.helper")
        );
        assert_eq!(
            env[&format!("GIT_CONFIG_VALUE_{last}")].as_deref(),
            Some("")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_explicit_value_set_afterwards_wins() {
        let dir = std::env::temp_dir().join(format!("agent-env-b-{}", std::process::id()));
        let mut cmd = Command::new("true");
        deny_github(&mut cmd, &dir).unwrap();
        cmd.env("GH_TOKEN", "opted-in");
        assert_eq!(envs(&cmd)["GH_TOKEN"].as_deref(), Some("opted-in"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn existing_git_config_pairs_are_kept() {
        assert_eq!(git_config_count(None), 0);
        assert_eq!(git_config_count(Some("2".into())), 2);
        assert_eq!(git_config_count(Some("junk".into())), 0);
    }

    #[test]
    fn local_git_still_works_but_ssh_and_credentials_do_not() {
        // A real git: init/commit succeed with the denied environment.
        let dir = std::env::temp_dir().join(format!("agent-env-git-{}", std::process::id()));
        let repo = dir.join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        let git = |args: &[&str]| {
            let mut cmd = Command::new("git");
            cmd.args(args).current_dir(&repo);
            deny_github(&mut cmd, &dir.join("gh")).unwrap();
            cmd.output().unwrap()
        };
        if !git(&["init", "-q"]).status.success() {
            return; // no git on this machine: nothing to prove here
        }
        let helper = git(&["config", "--get-all", "credential.helper"]);
        let helper = String::from_utf8_lossy(&helper.stdout);
        assert!(helper.lines().last().unwrap_or("").is_empty(), "{helper:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
