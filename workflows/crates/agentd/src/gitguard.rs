//! The git guard every run gets: no direct push to main on a protected org.
//!
//! Headless agents push branches; a push straight to `main` or `master` on
//! a protected org's repo must be impossible from any run, script or model,
//! however the prompt or script is written. Which orgs are protected is
//! configuration (`protected_orgs:` in `~/.config/agentd/agentd.yaml`, see
//! `agent_core::guard`); none are by default, and then only the bypass
//! refusals below apply. Two layers, written into the
//! state directory on startup (like `log.sh`):
//!
//! - `lib/githooks/<hook>` — one dispatcher (`assets/githook.sh`) under every
//!   hook name. Its `pre-push` refuses `refs/heads/main|master` on a protected
//!   org's remote; every hook then runs the repo's own hook of that name, so
//!   repo secret scans and lint still run.
//! - `lib/bin/git` — a shim first on every run's PATH. It points
//!   `core.hooksPath` at the dispatcher on every call and refuses the ways
//!   around a hook (`push --no-verify`, overriding `core.hooksPath`).
//!
//! The run's env also carries the hooksPath override, so a run whose shell
//! rebuilds PATH still gets the hook. GitHub branch protection is the last
//! line; this one makes the attempt fail locally, loudly, in the run's log.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use agent_core::paths;
use anyhow::{Context, Result};

const GITHOOK_SH: &str = include_str!("../assets/githook.sh");
const GIT_SHIM_SH: &str = include_str!("../assets/git-shim.sh");

/// Every hook git can run on a client, bar `reference-transaction` (it
/// fires on every ref update and would only slow runs down).
const HOOKS: &[&str] = &[
    "applypatch-msg",
    "pre-applypatch",
    "post-applypatch",
    "pre-commit",
    "pre-merge-commit",
    "prepare-commit-msg",
    "commit-msg",
    "post-commit",
    "pre-rebase",
    "post-checkout",
    "post-merge",
    "pre-push",
    "post-rewrite",
    "pre-auto-gc",
    "push-to-checkout",
];

fn lib_dir() -> Result<PathBuf> {
    Ok(paths::state_dir().context("resolve state dir")?.join("lib"))
}

/// The hook dispatcher directory, for `core.hooksPath`.
pub(crate) fn hooks_dir() -> Result<PathBuf> {
    Ok(lib_dir()?.join("githooks"))
}

/// The directory holding the `git` shim, for the front of PATH.
pub(crate) fn bin_dir() -> Result<PathBuf> {
    Ok(lib_dir()?.join("bin"))
}

fn write_exec(path: &Path, body: &str) -> Result<()> {
    std::fs::write(path, body).with_context(|| format!("write {}", path.display()))?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .with_context(|| format!("chmod {}", path.display()))
}

pub(crate) fn install() -> Result<()> {
    let hooks = hooks_dir()?;
    let bin = bin_dir()?;
    for dir in [&hooks, &bin] {
        std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
    }
    for name in HOOKS {
        write_exec(&hooks.join(name), GITHOOK_SH)?;
    }
    write_exec(&bin.join("git"), GIT_SHIM_SH)
}

#[cfg(test)]
// Tests should fail loudly on a broken fixture; unwrapping is the clearest way.
#[allow(clippy::unwrap_used)]
mod tests {
    use std::process::Command;

    use super::{write_exec, GITHOOK_SH, GIT_SHIM_SH, HOOKS};

    /// A scratch layout: lib/bin/git, lib/githooks/*, a bare "remote" whose
    /// URL looks like a GitHub repo of `org`, and a clone of it.
    fn setup(tag: &str, org: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        let root =
            std::env::temp_dir().join(format!("agentd-gitguard-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (bin, hooks) = (root.join("lib/bin"), root.join("lib/githooks"));
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::create_dir_all(&hooks).unwrap();
        for name in HOOKS {
            write_exec(&hooks.join(name), GITHOOK_SH).unwrap();
        }
        write_exec(&bin.join("git"), GIT_SHIM_SH).unwrap();
        let remote = root.join(format!("github.com/{org}/repo.git"));
        let clone = root.join("clone");
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
        (bin, clone)
    }

    fn git(bin: &std::path::Path, clone: &std::path::Path, args: &str) -> (bool, String) {
        let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap());
        let out = Command::new("sh")
            .arg("-c")
            .arg(format!("git {args}"))
            .current_dir(clone)
            .env("PATH", path)
            .env("AGENTD_PROTECTED_ORGS", "acme")
            .output()
            .unwrap();
        (
            out.status.success(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
        )
    }

    #[test]
    fn a_protected_org_never_takes_a_push_to_main() {
        let (bin, clone) = setup("protected", "acme");
        for refspec in ["main", "HEAD:main", "HEAD:refs/heads/master"] {
            let (ok, err) = git(&bin, &clone, &format!("push -q origin {refspec}"));
            assert!(!ok && err.contains("BLOCKED push"), "{refspec}: {err}");
        }
        // The ways around the hook are refused too.
        assert!(git(&bin, &clone, "push --no-verify origin main")
            .1
            .contains("BLOCKED git push --no-verify"));
        assert!(
            git(&bin, &clone, "-c core.hooksPath=/dev/null push origin main")
                .1
                .contains("BLOCKED overriding")
        );
        // A branch is fine.
        let (ok, err) = git(&bin, &clone, "push -q origin HEAD:refs/heads/abc-1-fix");
        assert!(ok, "{err}");
    }

    #[test]
    fn other_orgs_push_to_main_and_repo_hooks_still_run() {
        let (bin, clone) = setup("open", "jdoe");
        // The repo's own pre-push hook runs after the guard, with its stdin.
        let own = clone.join(".git/hooks/pre-push");
        std::fs::write(
            &own,
            "#!/bin/sh\ncat > \"$(git rev-parse --git-dir)/saw-push\"\n",
        )
        .unwrap();
        std::fs::set_permissions(&own, std::os::unix::fs::PermissionsExt::from_mode(0o755))
            .unwrap();
        let (ok, err) = git(&bin, &clone, "push -q origin main");
        assert!(ok, "{err}");
        let saw = std::fs::read_to_string(clone.join(".git/saw-push")).unwrap();
        assert!(saw.contains("refs/heads/main"), "{saw}");
    }
}
