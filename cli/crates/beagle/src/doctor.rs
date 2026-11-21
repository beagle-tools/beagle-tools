//! `beagle doctor`: finds the old standalone binaries this command
//! replaces that are still on `PATH` — `scout`, `recap`, `standup`, the
//! agentd stack (`agentctl`, `agenttop`, `agentd`), and another `beagle`
//! (the standalone rca 0.x binary, or a second copy of this one) that shadows or duplicates the running `beagle`.
//!
//! On macOS it also reports whether the `beagle serve` launchd agent is
//! running (`launchctl print`, read-only), and it runs the setup checks in
//! `health` (gh, agent services, tmux, skills, scout hooks), which only
//! warn: the exit code is non-zero only for old binaries or a down service.
//!
//! The PATH scan only reads directory entries; it never runs what it finds.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Binaries that used to ship on their own, and the `beagle` command that
/// replaces each.
const RETIRED: &[(&str, &str)] = &[
    ("scout", "beagle scout"),
    ("recap", "beagle recap"),
    ("standup", "beagle standup"),
    ("agentctl", "beagle agent"),
    ("agenttop", "beagle agent top"),
    ("agentd", "beagle agent daemon run"),
    // `beagle-agentd` is not here: the beagle formula still ships it for one
    // release so rca services installed before `beagle rca agentd run` keep
    // working. `health.rs` warns while a service still runs it.
];

/// The `beagle` command that replaces the retired binary `name`.
fn replacement(name: &str) -> &'static str {
    RETIRED
        .iter()
        .find(|(old, _)| *old == name)
        .map_or("beagle", |(_, now)| now)
}

/// One binary on `PATH` worth telling the user about.
#[derive(Debug, PartialEq, Eq)]
pub enum Finding {
    /// A standalone tool binary now built into `beagle`.
    Retired {
        /// Its name, e.g. `scout`.
        name: &'static str,
        /// Where it is.
        path: PathBuf,
    },
    /// Another `beagle` that runs instead of this one (earlier on `PATH`).
    Shadowing(PathBuf),
    /// Another `beagle` later on `PATH`: harmless now, confusing later.
    Duplicate(PathBuf),
}

/// `beagle doctor` (no arguments).
pub fn run() -> ExitCode {
    // Not knowing where we run from only weakens the report: every other
    // `beagle` is then listed as a duplicate rather than a shadow.
    let this = std::env::current_exe().ok().map(|p| resolve(&p));
    // No PATH means nothing on it to report.
    let path_var = std::env::var_os("PATH").unwrap_or_default();
    let findings = scan(&path_var, this.as_deref());
    let serve_ok = report_serve();
    // Setup checks only warn: each matters to some tools, not all.
    if let Some(home) = beagle_utils::paths::home_dir() {
        for check in crate::health::checks(&home, &path_var) {
            println!("{}", check.line());
        }
    }
    if findings.is_empty() {
        println!("✓ no old standalone binaries on PATH — `beagle` is the only one");
        return if serve_ok {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        };
    }
    for finding in &findings {
        println!("{}", describe(finding));
    }
    println!("\nRemove them with whatever installed them (`brew uninstall …`, `cargo uninstall …`, or `rm`).");
    ExitCode::FAILURE
}

/// Prints whether the `beagle serve` background service is running (macOS
/// only). Returns `false` only for a service that is installed but down:
/// not installing it is a choice, not a problem.
fn report_serve() -> bool {
    if !cfg!(target_os = "macos") {
        return true;
    }
    let (line, ok) = match beagle_serve::service::status() {
        Ok(status) => serve_line(&status),
        Err(e) => (format!("? beagle serve: could not ask launchd: {e}"), true),
    };
    println!("{line}");
    ok
}

/// The doctor's line for a service status, and whether it is healthy.
pub fn serve_line(status: &beagle_serve::service::Status) -> (String, bool) {
    use beagle_serve::service::Status;
    match status {
        Status::NotInstalled => (format!("· {}", status.describe()), true),
        Status::Loaded { pid: Some(_), .. } => (format!("✓ {}", status.describe()), true),
        Status::Loaded { pid: None, .. } | Status::NotLoaded(_) => {
            (format!("✗ {}", status.describe()), false)
        }
    }
}

/// Every [`Finding`] in the `PATH`-style `path_var`, in `PATH` order.
/// `this` is the running binary, resolved through symlinks.
pub fn scan(path_var: &OsStr, this: Option<&Path>) -> Vec<Finding> {
    let mut findings = Vec::new();
    let mut seen_self = false;
    let mut seen = Vec::new();
    for dir in std::env::split_paths(path_var) {
        let beagle = dir.join("beagle");
        if is_executable(&beagle) {
            let resolved = resolve(&beagle);
            if Some(resolved.as_path()) == this {
                seen_self = true;
            } else if !seen.contains(&resolved) {
                findings.push(if seen_self || this.is_none() {
                    Finding::Duplicate(beagle)
                } else {
                    Finding::Shadowing(beagle)
                });
            }
            seen.push(resolved);
        }
        for (name, _) in RETIRED {
            let path = dir.join(name);
            if is_executable(&path) && !seen.contains(&resolve(&path)) {
                seen.push(resolve(&path));
                findings.push(Finding::Retired { name, path });
            }
        }
    }
    findings
}

fn describe(finding: &Finding) -> String {
    match finding {
        Finding::Retired { name, path } => format!(
            "• {} — the old standalone `{name}`; it is `{}` now",
            path.display(),
            replacement(name)
        ),
        Finding::Shadowing(path) => format!(
            "• {} — another `beagle` EARLIER on PATH runs instead of this one; likely \
             the old standalone rca binary (0.x), which `beagle rca` replaces",
            path.display()
        ),
        Finding::Duplicate(path) => format!(
            "• {} — another `beagle` later on PATH (an old standalone rca binary, or a \
             second copy of this one)",
            path.display()
        ),
    }
}

/// `path` through symlinks; unresolvable paths compare as given.
fn resolve(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

#[cfg(test)]
#[path = "doctor_tests.rs"]
mod tests;
