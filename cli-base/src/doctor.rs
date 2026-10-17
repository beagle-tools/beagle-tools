//! `beagle doctor`: checks the install and reports each finding as
//! `ok` / `warn` / `fail`. Any `fail` makes the command exit 1.

use std::ffi::OsStr;
use std::fmt;
use std::path::{Path, PathBuf};

use crate::install::is_executable_file;
use crate::paths::Layout;
use crate::registry::ExtensionSpec;

/// Binaries replaced by extensions; finding one on `PATH` deserves a warning.
pub const LEGACY_BINS: &[&str] = &[
    "beagle-rca",
    "beagle-agentd",
    "agentctl",
    "agenttop",
    "agentd",
];

/// How bad a finding is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Status {
    /// Fine.
    Ok,
    /// Works, but worth fixing.
    Warn,
    /// Broken.
    Fail,
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Ok => "ok  ",
            Self::Warn => "warn",
            Self::Fail => "fail",
        })
    }
}

/// One finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    /// Severity.
    pub status: Status,
    /// What it is about, and what to do.
    pub message: String,
}

fn check(status: Status, message: impl Into<String>) -> Check {
    Check {
        status,
        message: message.into(),
    }
}

/// What the checks look at; every path is injectable for tests.
#[derive(Debug, Clone)]
pub struct Inputs<'a> {
    /// The data layout.
    pub layout: &'a Layout,
    /// The extensions to inspect.
    pub registry: &'a [ExtensionSpec],
    /// `$PATH`.
    pub path_var: &'a OsStr,
    /// This binary.
    pub exe: &'a Path,
    /// `$HOME`, for launchd/systemd units.
    pub home: Option<&'a Path>,
}

/// Runs every check.
pub fn run(inputs: &Inputs<'_>) -> Vec<Check> {
    let mut checks = vec![writable(inputs.layout)];
    checks.extend(extensions(inputs.layout, inputs.registry));
    checks.push(on_path(inputs.path_var, inputs.exe));
    checks.extend(legacy(inputs.path_var, inputs.layout.root()));
    if let Some(home) = inputs.home {
        checks.extend(units(home));
    }
    checks
}

fn writable(layout: &Layout) -> Check {
    let dir = layout.extensions_dir();
    let probe = dir.join(format!(".doctor-{}", std::process::id()));
    let result = std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&probe, b""));
    let _ = std::fs::remove_file(&probe);
    match result {
        Ok(()) => check(Status::Ok, format!("{} is writable", dir.display())),
        Err(e) => check(
            Status::Fail,
            format!("{} is not writable: {e}", dir.display()),
        ),
    }
}

fn extensions(layout: &Layout, registry: &[ExtensionSpec]) -> Vec<Check> {
    let mut checks = Vec::new();
    for spec in registry {
        let link = layout.current_link(spec.name);
        if std::fs::symlink_metadata(&link).is_err() {
            continue; // not installed
        }
        let Some(version) = layout.current_version(spec.name) else {
            checks.push(check(
                Status::Fail,
                format!(
                    "{}: `current` does not resolve — run: beagle extensions install {}",
                    spec.name, spec.name
                ),
            ));
            continue;
        };
        let bin = match crate::dispatch::installed(spec, layout) {
            Ok(resolved) => resolved.bin().to_path_buf(),
            Err(e) => {
                checks.push(check(Status::Fail, format!("{} {version}: {e}", spec.name)));
                continue;
            }
        };
        if is_executable_file(&bin) {
            checks.push(check(
                Status::Ok,
                format!("{} {version} ({})", spec.name, bin.display()),
            ));
        } else {
            checks.push(check(
                Status::Fail,
                format!("{} {version}: {} is not executable — run: beagle extensions install {} --version {version}", spec.name, bin.display(), spec.name),
            ));
        }
    }
    checks
}

fn on_path(path_var: &OsStr, exe: &Path) -> Check {
    let found = beagle_utils::process::which_in("beagle", path_var);
    let canon = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    match found {
        Some(found) if canon(&found) == canon(exe) => {
            check(Status::Ok, "beagle on PATH is this binary")
        }
        Some(found) => check(
            Status::Warn,
            format!(
                "beagle on PATH is {}, not this binary ({})",
                found.display(),
                exe.display()
            ),
        ),
        None => check(
            Status::Warn,
            format!("beagle is not on PATH (this binary: {})", exe.display()),
        ),
    }
}

fn legacy(path_var: &OsStr, data_root: &Path) -> Vec<Check> {
    LEGACY_BINS
        .iter()
        .filter_map(|bin| beagle_utils::process::which_in(bin, path_var))
        .filter(|found| !found.starts_with(data_root))
        .map(|found| {
            check(
                Status::Warn,
                format!(
                    "old binary {} is on PATH — it is replaced by a beagle extension",
                    found.display()
                ),
            )
        })
        .collect()
}

fn units(home: &Path) -> Vec<Check> {
    let mut found: Vec<(PathBuf, Option<String>)> = Vec::new();
    let agents = home.join("Library/LaunchAgents");
    for path in files(&agents, |n| {
        (n.starts_with("com.beagle.") || n.starts_with("com.agentd.")) && has_ext(n, "plist")
    }) {
        let program = std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| plist_program(&t));
        found.push((path, program));
    }
    let systemd = home.join(".config/systemd/user");
    for path in files(&systemd, |n| {
        n.starts_with("beagle-") && has_ext(n, "service")
    }) {
        let program = std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| unit_exec(&t));
        found.push((path, program));
    }
    found
        .into_iter()
        .map(|(unit, program)| match program {
            Some(p) if Path::new(&p).exists() => {
                check(Status::Ok, format!("{} runs {p}", unit.display()))
            }
            Some(p) => check(
                Status::Fail,
                format!("{} runs {p}, which does not exist", unit.display()),
            ),
            None => check(
                Status::Warn,
                format!("{}: could not find the program it runs", unit.display()),
            ),
        })
        .collect()
}

fn has_ext(name: &str, ext: &str) -> bool {
    Path::new(name)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case(ext))
}

fn files(dir: &Path, keep: impl Fn(&str) -> bool) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(std::result::Result::ok)
        .filter(|e| e.file_name().to_str().is_some_and(&keep))
        .map(|e| e.path())
        .collect();
    paths.sort();
    paths
}

/// The first `ProgramArguments` string of a launchd plist.
pub fn plist_program(text: &str) -> Option<String> {
    let after = &text[text.find("<key>ProgramArguments</key>")?..];
    let start = after.find("<string>")? + "<string>".len();
    let end = after[start..].find("</string>")?;
    Some(after[start..start + end].trim().to_owned())
}

/// The program of a systemd unit's `ExecStart=` line.
pub fn unit_exec(text: &str) -> Option<String> {
    let line = text
        .lines()
        .find_map(|l| l.trim().strip_prefix("ExecStart="))?;
    // Prefixes like `-` or `@` modify behaviour; they aren't the path.
    let program = line.trim().trim_start_matches(['-', '@', '+', '!', ':']);
    program.split_whitespace().next().map(str::to_owned)
}

#[cfg(test)]
#[path = "tests/doctor.rs"]
mod tests;
