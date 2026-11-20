//! The git guard every run gets: no direct push to main.
//!
//! Headless agents push branches; a push straight to `main` or `master`
//! must be impossible from any run, script or model, however the prompt or
//! script is written. By default every remote is guarded;
//! `protected_orgs:` in `~/.config/agentd/agentd.yaml` (see
//! `agent_core::guard`) narrows it to those orgs. The policy is written
//! INTO the hook file when agentd starts — never read from a run's
//! environment, which the run controls. Two layers, written into the state
//! directory on startup (like `log.sh`):
//!
//! - `lib/githooks/<hook>` — one dispatcher (`assets/githook.sh`) under every
//!   hook name. Its `pre-push` refuses `refs/heads/main|master` on a guarded
//!   remote (resolved with `git ls-remote --get-url`, any host); every hook
//!   then runs the repo's own hook of that name, so repo secret scans and
//!   lint still run.
//! - `lib/bin/git` — a shim first on every run's PATH. It points
//!   `core.hooksPath` at the dispatcher on every call and refuses the ways
//!   around a hook (`push --no-verify`, overriding `core.hooksPath`).
//!
//! The run's env also appends the hooksPath override to its `GIT_CONFIG_*`
//! pairs, so a run whose shell rebuilds PATH still gets the hook. GitHub branch protection is the last
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

/// The hook dispatcher with the push policy baked in: `*` (every remote)
/// when `protected` is empty, else the org names. Anything but
/// `[A-Za-z0-9._-]` is dropped from a name, so config can never inject
/// shell into the hook.
pub(crate) fn hook_body(protected: &[String]) -> String {
    let orgs: Vec<String> = protected
        .iter()
        .map(|o| {
            o.chars()
                .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
                .collect::<String>()
        })
        .filter(|o| !o.is_empty())
        .collect();
    let policy = if orgs.is_empty() {
        "*".to_owned()
    } else {
        orgs.join(" ")
    };
    GITHOOK_SH.replace("__AGENTD_PROTECTED_ORGS__", &policy)
}

/// Writes the hook dispatcher (with `protected` baked in, see
/// [`hook_body`]) and the git shim into the state directory.
pub(crate) fn install(protected: &[String]) -> Result<()> {
    let hooks = hooks_dir()?;
    let bin = bin_dir()?;
    for dir in [&hooks, &bin] {
        std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
    }
    let hook = hook_body(protected);
    for name in HOOKS {
        write_exec(&hooks.join(name), &hook)?;
    }
    write_exec(&bin.join("git"), GIT_SHIM_SH)
}

#[cfg(test)]
#[path = "tests/gitguard.rs"]
mod tests;
