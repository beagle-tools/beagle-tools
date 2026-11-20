//! `beagle doctor`'s setup checks: what the tools need around them — `gh`
//! signed in, `beagle-agentd` and `tmux` installed, agent skills current,
//! scout's hooks in place. Every problem here is a warning with the fix,
//! never a failure: each is only needed by some tools.

use std::ffi::OsStr;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use beagle_utils::process;

/// How long a probe (`gh auth status`, `beagle-agentd --version`) may take.
const PROBE_TIMEOUT: Duration = Duration::from_secs(10);

/// One check's result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Check {
    /// Fine.
    Ok(String),
    /// Missing or stale, with the fix.
    Warn(String),
}

impl Check {
    /// The line `beagle doctor` prints.
    #[must_use]
    pub fn line(&self) -> String {
        match self {
            Self::Ok(s) => format!("✓ {s}"),
            Self::Warn(s) => format!("⚠ {s}"),
        }
    }
}

/// Every setup check for `home`, with programs looked up on `path_var`.
#[must_use]
pub fn checks(home: &Path, path_var: &OsStr) -> Vec<Check> {
    vec![
        gh(path_var),
        agentd(path_var),
        tmux(path_var),
        skills(home),
        scout_hooks(home),
    ]
}

/// `gh` installed and signed in.
fn gh(path_var: &OsStr) -> Check {
    let Some(gh) = process::which_in("gh", path_var) else {
        return Check::Warn(
            "gh is not installed — recap, standup and tandem need it (https://cli.github.com)"
                .to_owned(),
        );
    };
    let out =
        process::output_with_timeout(Command::new(gh).args(["auth", "status"]), PROBE_TIMEOUT);
    if out.is_ok_and(|o| o.status.success()) {
        Check::Ok("gh is signed in".to_owned())
    } else {
        Check::Warn("gh is not signed in — run `gh auth login`".to_owned())
    }
}

/// `beagle-agentd` (rca's agent daemon) on PATH, and its version.
fn agentd(path_var: &OsStr) -> Check {
    let Some(agentd) = process::which_in("beagle-agentd", path_var) else {
        return Check::Warn(
            "beagle-agentd is not on PATH — RCA agents (`beagle rca agent …`) need it; \
             `brew install beagle-tools/tap/beagle` installs it"
                .to_owned(),
        );
    };
    let version =
        process::output_with_timeout(Command::new(&agentd).arg("--version"), PROBE_TIMEOUT)
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
            .filter(|v| !v.is_empty());
    Check::Ok(match version {
        Some(v) => format!("{v} at {}", agentd.display()),
        None => format!("beagle-agentd at {}", agentd.display()),
    })
}

/// `tmux` on PATH.
fn tmux(path_var: &OsStr) -> Check {
    if process::which_in("tmux", path_var).is_some() {
        Check::Ok("tmux is installed".to_owned())
    } else {
        Check::Warn("tmux is not installed — scheduled agent runs (agentd) need it".to_owned())
    }
}

/// rca's and scout's agent skills match this binary's copies.
fn skills(home: &Path) -> Check {
    let mut stale = Vec::new();
    if !crate::setup::rca_skills_current(home) {
        stale.push("rca");
    }
    if !crate::setup::scout_skills_current(home) {
        stale.push("scout");
    }
    if stale.is_empty() {
        Check::Ok("agent skills are current".to_owned())
    } else {
        Check::Warn(format!(
            "{} agent skills are missing or stale — run `beagle setup`",
            stale.join(" and ")
        ))
    }
}

/// scout's Claude Code hooks are installed.
fn scout_hooks(home: &Path) -> Check {
    if crate::setup::scout_hooks_installed(home) {
        Check::Ok("scout's session hooks are installed".to_owned())
    } else {
        Check::Warn(
            "scout's session hooks are not installed — run `beagle setup` \
             (or `beagle scout hook install`)"
                .to_owned(),
        )
    }
}

#[cfg(test)]
#[path = "health_tests.rs"]
mod tests;
