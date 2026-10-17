//! `beagle <name> args…` → the extension's own binary.
//!
//! Resolution order: reserved base command → registry (installed or not)
//! → `beagle-<name>` on `PATH` (the dev escape hatch) → unknown. The base
//! then *execs* the binary, so signals, the TTY and the exit code behave as
//! if the user had run it directly. The env it sets is a public contract
//! (see `AGENTS.md`).

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{Error, Result};
use crate::install::MANIFEST;
use crate::manifest::ExtensionManifest;
use crate::paths::Layout;
use crate::registry::{self, ExtensionSpec, RESERVED};
use crate::version::Version;

/// What `beagle <name>` resolved to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolved {
    /// An installed registry extension.
    Installed {
        /// Canonical name.
        name: String,
        /// Active version.
        version: Version,
        /// `extensions/<name>/<version>/`.
        dir: PathBuf,
        /// The entrypoint to exec.
        bin: PathBuf,
    },
    /// A `beagle-<name>` found on `PATH`.
    OnPath {
        /// The name as typed.
        name: String,
        /// The binary.
        bin: PathBuf,
    },
}

impl Resolved {
    /// The binary to exec.
    pub fn bin(&self) -> &Path {
        match self {
            Self::Installed { bin, .. } | Self::OnPath { bin, .. } => bin,
        }
    }

    /// The extension name to report in errors.
    pub fn name(&self) -> &str {
        match self {
            Self::Installed { name, .. } | Self::OnPath { name, .. } => name,
        }
    }
}

/// Resolves `name` (see the module docs). `path_var` is `$PATH`.
pub fn resolve(
    name: &str,
    registry: &[ExtensionSpec],
    layout: &Layout,
    path_var: &OsStr,
) -> Result<Resolved> {
    if registry::is_reserved(name) {
        return Err(Error::Reserved {
            name: name.to_owned(),
        });
    }
    if let Some(spec) = registry::find(registry, name) {
        return installed(spec, layout);
    }
    let program = format!("beagle-{name}");
    if let Some(bin) = beagle_utils::process::which_in(&program, path_var) {
        return Ok(Resolved::OnPath {
            name: name.to_owned(),
            bin,
        });
    }
    Err(Error::UnknownCommand {
        name: name.to_owned(),
        hint: did_you_mean(name, registry),
    })
}

/// The active install of `spec`, or [`Error::NotInstalled`].
pub fn installed(spec: &ExtensionSpec, layout: &Layout) -> Result<Resolved> {
    let not_installed = || Error::NotInstalled {
        name: spec.name.to_owned(),
    };
    let version = layout
        .current_version(spec.name)
        .ok_or_else(not_installed)?;
    let dir = layout.version_dir(spec.name, version);
    let manifest = ExtensionManifest::load(&dir.join(MANIFEST))?;
    Ok(Resolved::Installed {
        name: spec.name.to_owned(),
        version,
        // Through `current`, as the contract documents; exec resolves it once.
        bin: layout.active_bin(spec.name, &manifest.bin),
        dir,
    })
}

/// The command that runs `resolved` with `args`, carrying the dispatch env.
/// `base` is this binary's path; `typed` is the name as the user typed it.
pub fn command(resolved: &Resolved, args: &[OsString], base: &Path, typed: &str) -> Command {
    let mut cmd = Command::new(resolved.bin());
    cmd.args(args)
        .env("BEAGLE_BASE", base)
        .env("BEAGLE_BASE_VERSION", Version::current().to_string())
        .env("BEAGLE_EXTENSION", resolved.name())
        .env("BEAGLE_INVOKED_AS", format!("beagle {typed}"));
    match resolved {
        Resolved::Installed { version, dir, .. } => {
            cmd.env("BEAGLE_EXTENSION_VERSION", version.to_string())
                .env("BEAGLE_EXTENSION_DIR", dir);
        }
        // A dev binary has no installed version or directory; don't leak
        // the parent's values either.
        Resolved::OnPath { .. } => {
            cmd.env_remove("BEAGLE_EXTENSION_VERSION")
                .env_remove("BEAGLE_EXTENSION_DIR");
        }
    }
    cmd
}

/// Replaces this process with `cmd`. Returns only on failure.
pub fn exec(mut cmd: Command, resolved: &Resolved) -> Error {
    #[cfg(unix)]
    let source = std::os::unix::process::CommandExt::exec(&mut cmd);
    #[cfg(not(unix))]
    let source = {
        let _ = &mut cmd;
        std::io::Error::new(std::io::ErrorKind::Unsupported, "needs unix")
    };
    Error::Exec {
        path: resolved.bin().to_path_buf(),
        name: resolved.name().to_owned(),
        source,
    }
}

/// ` — did you mean "rca"?` for the closest base command or extension
/// within two edits, else empty.
pub fn did_you_mean(name: &str, registry: &[ExtensionSpec]) -> String {
    let candidates = RESERVED
        .iter()
        .copied()
        .filter(|c| !matches!(*c, "completions" | "self"))
        .chain(registry.iter().map(|s| s.name));
    candidates
        .map(|c| (edit_distance(name, c), c))
        .filter(|(d, _)| *d <= 2)
        .min()
        .map(|(_, c)| format!(" — did you mean \"{c}\"?"))
        .unwrap_or_default()
}

/// Levenshtein distance over chars.
fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut prev = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cur = row[j + 1];
            row[j + 1] = (prev + usize::from(ca != *cb)).min(row[j] + 1).min(cur + 1);
            prev = cur;
        }
    }
    row[b.len()]
}

#[cfg(test)]
#[path = "tests/dispatch.rs"]
mod tests;
