//! Tests for the git guard: real git, a scratch remote, and the installed
//! hook + shim, exactly as a run sees them.

// Tests should fail loudly on a broken fixture; unwrapping is the clearest way.
#![allow(clippy::unwrap_used)]

use std::path::{Path, PathBuf};
use std::process::Command;

use super::{hook_body, write_exec, GIT_SHIM_SH, HOOKS};
use crate::runenv::git_config_append;

/// A scratch layout: lib/bin/git, lib/githooks/* (policy from `protected`),
/// a bare "remote" whose URL looks like a GitHub repo of `org`, and a clone.
fn setup(tag: &str, org: &str, protected: &[&str]) -> (PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!("agentd-gitguard-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (bin, hooks) = (root.join("lib/bin"), root.join("lib/githooks"));
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::create_dir_all(&hooks).unwrap();
    let protected: Vec<String> = protected.iter().map(|s| (*s).to_owned()).collect();
    let hook = hook_body(&protected);
    for name in HOOKS {
        write_exec(&hooks.join(name), &hook).unwrap();
    }
    write_exec(&bin.join("git"), GIT_SHIM_SH).unwrap();
    let remote = root.join(format!("github.com/{org}/repo.git"));
    let sh = |cmd: &str| {
        let ok = Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .current_dir(&root)
            .status()
            .unwrap();
        assert!(ok.success(), "{cmd}");
    };
    sh(&format!(
        "git init -q --bare -b main '{}'",
        remote.display()
    ));
    sh(&format!(
        "git clone -q 'file://{}' clone 2>/dev/null; cd clone && git checkout -q -b main && \
         git -c user.email=t@t -c user.name=t commit -q --allow-empty -m init",
        remote.display()
    ));
    (bin, root.join("clone"))
}

/// Runs `git <args>` in `clone` through the shim, with `env` added.
fn git(bin: &Path, clone: &Path, args: &str, env: &[(&str, &str)]) -> (bool, String) {
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap());
    let mut cmd = Command::new("sh");
    cmd.arg("-c")
        .arg(format!("git {args}"))
        .current_dir(clone)
        .env("PATH", path);
    for (k, v) in env {
        cmd.env(k, v);
    }
    let out = cmd.output().unwrap();
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn by_default_no_remote_takes_a_push_to_main() {
    let (bin, clone) = setup("default", "jdoe", &[]);
    for refspec in ["main", "HEAD:main", "HEAD:refs/heads/master"] {
        let (ok, err) = git(&bin, &clone, &format!("push -q origin {refspec}"), &[]);
        assert!(!ok && err.contains("BLOCKED push"), "{refspec}: {err}");
    }
    // A branch is fine.
    let (ok, err) = git(
        &bin,
        &clone,
        "push -q origin HEAD:refs/heads/abc-1-fix",
        &[],
    );
    assert!(ok, "{err}");
}

#[test]
fn clearing_the_env_var_no_longer_disables_the_guard() {
    let (bin, clone) = setup("env", "jdoe", &[]);
    let (ok, err) = git(
        &bin,
        &clone,
        "push -q origin main",
        &[("AGENTD_PROTECTED_ORGS", ""), ("GIT_CONFIG_COUNT", "0")],
    );
    assert!(!ok && err.contains("BLOCKED push"), "{err}");
}

#[test]
fn a_protected_org_list_narrows_the_guard() {
    let (bin, clone) = setup("protected", "acme", &["acme"]);
    let (ok, err) = git(&bin, &clone, "push -q origin main", &[]);
    assert!(!ok && err.contains("BLOCKED push"), "{err}");
    // The ways around the hook are refused too.
    assert!(git(&bin, &clone, "push --no-verify origin main", &[])
        .1
        .contains("BLOCKED git push --no-verify"));
    assert!(git(
        &bin,
        &clone,
        "-c core.hooksPath=/dev/null push origin main",
        &[]
    )
    .1
    .contains("BLOCKED overriding"));
    // Another org's repo is outside a narrowed guard.
    let (bin, clone) = setup("other", "jdoe", &["acme"]);
    let (ok, err) = git(&bin, &clone, "push -q origin main", &[]);
    assert!(ok, "{err}");
}

#[test]
fn an_org_is_matched_through_a_rewritten_remote_url() {
    // `insteadOf` (like an SSH host alias) hides the real URL from the
    // name; the hook resolves it with `git ls-remote --get-url`.
    let (bin, clone) = setup("alias", "acme", &["acme"]);
    let url = String::from_utf8(
        Command::new("git")
            .args(["remote", "get-url", "origin"])
            .current_dir(&clone)
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    let url = url.trim();
    let alias = "work-remote:";
    let ok = Command::new("git")
        .args(["config", &format!("url.{url}.insteadOf"), alias])
        .current_dir(&clone)
        .status()
        .unwrap();
    assert!(ok.success());
    let (ok, err) = git(&bin, &clone, &format!("push -q {alias} main"), &[]);
    assert!(!ok && err.contains("BLOCKED push"), "{err}");
}

#[test]
fn the_users_git_config_env_is_kept() {
    let (bin, clone) = setup("cfg", "jdoe", &["acme"]);
    let (ok, out) = git(
        &bin,
        &clone,
        "config --get agentd.test 1>&2",
        &[
            ("GIT_CONFIG_COUNT", "1"),
            ("GIT_CONFIG_KEY_0", "agentd.test"),
            ("GIT_CONFIG_VALUE_0", "kept"),
        ],
    );
    assert!(ok && out.trim() == "kept", "{out}");
    // The run env appends the same way.
    let lines = git_config_append(Path::new("/state/lib/githooks"));
    let script = format!(
        "GIT_CONFIG_COUNT=1; GIT_CONFIG_KEY_0=user.name; GIT_CONFIG_VALUE_0=me; \
         export GIT_CONFIG_COUNT GIT_CONFIG_KEY_0 GIT_CONFIG_VALUE_0\n{lines}\
         echo \"$GIT_CONFIG_COUNT $GIT_CONFIG_KEY_0 $GIT_CONFIG_KEY_1 $GIT_CONFIG_VALUE_1\""
    );
    let out = Command::new("sh").arg("-c").arg(script).output().unwrap();
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "2 user.name core.hooksPath /state/lib/githooks"
    );
}

#[test]
fn the_policy_is_sanitised_into_the_hook() {
    assert!(hook_body(&[]).contains("PROTECTED_ORGS='*'"));
    let narrowed = hook_body(&["acme".to_owned(), "wid'gets;rm".to_owned()]);
    assert!(
        narrowed.contains("PROTECTED_ORGS='acme widgetsrm'"),
        "{narrowed}"
    );
}

#[test]
fn other_hooks_of_the_repo_still_run() {
    let (bin, clone) = setup("own", "jdoe", &["acme"]);
    // The repo's own pre-push hook runs after the guard, with its stdin.
    let own = clone.join(".git/hooks/pre-push");
    std::fs::write(
        &own,
        "#!/bin/sh\ncat > \"$(git rev-parse --git-dir)/saw-push\"\n",
    )
    .unwrap();
    std::fs::set_permissions(&own, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let (ok, err) = git(&bin, &clone, "push -q origin main", &[]);
    assert!(ok, "{err}");
    let saw = std::fs::read_to_string(clone.join(".git/saw-push")).unwrap();
    assert!(saw.contains("refs/heads/main"), "{saw}");
}
