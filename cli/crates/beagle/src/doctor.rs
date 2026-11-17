//! `beagle doctor`: finds the old standalone binaries this command
//! replaces that are still on `PATH` — `scout`, `recap`, `standup`, and
//! another `beagle` (the standalone rca 0.x binary, or a second copy of
//! this one) that shadows or duplicates the running `beagle`.
//!
//! It only reads directory entries; it never runs what it finds.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Tools that used to ship as their own binary and are now `beagle <name>`.
const RETIRED: &[&str] = &["scout", "recap", "standup"];

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
    if findings.is_empty() {
        println!("✓ no old standalone binaries on PATH — `beagle` is the only one");
        return ExitCode::SUCCESS;
    }
    for finding in &findings {
        println!("{}", describe(finding));
    }
    println!("\nRemove them with whatever installed them (`brew uninstall …`, `cargo uninstall …`, or `rm`).");
    ExitCode::FAILURE
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
        for name in RETIRED {
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
            "• {} — the old standalone `{name}`; it is `beagle {name}` now",
            path.display()
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
