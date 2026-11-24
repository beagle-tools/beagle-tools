//! `beagle doctor`'s setup checks: what the tools need around them — `gh`
//! signed in, agent services running `beagle`, `tmux` installed, agent skills current,
//! scout's hooks in place. Every problem here is a warning with the fix,
//! never a failure: each is only needed by some tools.

use std::ffi::OsStr;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use beagle_utils::process;

/// How long a probe (`gh auth status`) may take.
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
        agent_services(home),
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

/// The agent services installed before the daemons moved under `beagle`:
/// (unit file under `$HOME`, the retired binary it may still run, the
/// command that rewrites it).
const SERVICES: &[(&str, &str, &str)] = &[
    (
        "Library/LaunchAgents/com.agentd.daemon.plist",
        "agentd",
        "beagle agent daemon install",
    ),
    (
        "Library/LaunchAgents/com.beagle.agentd.plist",
        "beagle-agentd",
        "beagle rca agent install",
    ),
    (
        ".config/systemd/user/beagle-agentd.service",
        "beagle-agentd",
        "beagle rca agent install",
    ),
];

/// Whether a launchd plist or systemd unit runs the binary `name` (its
/// program is `…/<name>`), rather than `beagle`.
fn runs_retired(unit: &str, name: &str) -> bool {
    unit.contains(&format!("/{name}</string>"))
        || unit.lines().any(|line| {
            line.strip_prefix("ExecStart=")
                .and_then(|exec| exec.split_whitespace().next())
                .is_some_and(|program| program.ends_with(&format!("/{name}")))
        })
}

/// Agent services (agentd's, rca's) still running a retired binary, which
/// goes away in the next release.
fn agent_services(home: &Path) -> Check {
    let stale: Vec<String> = SERVICES
        .iter()
        .filter(|(unit, name, _)| {
            std::fs::read_to_string(home.join(unit)).is_ok_and(|text| runs_retired(&text, name))
        })
        .map(|(_, name, fix)| format!("`{name}` (run `{fix}`)"))
        .collect();
    if stale.is_empty() {
        Check::Ok("agent services run `beagle` (or none are installed)".to_owned())
    } else {
        Check::Warn(format!(
            "an agent service still runs {}, which goes away in the next release",
            stale.join(" and ")
        ))
    }
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
