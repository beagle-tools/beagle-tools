//! The on-disk layout under `$XDG_DATA_HOME/beagle` (a public format; see
//! `AGENTS.md`):
//!
//! ```text
//! extensions/<ext>/<version>/bin/…        the extension's binaries
//! extensions/<ext>/<version>/extension.toml, install.toml
//! extensions/<ext>/current -> <version>   relative symlink, swapped atomically
//! extensions/<ext>/.staging-<pid>/        an install in progress
//! state.toml                              cached update checks
//! locks/<ext>.lock                        advisory install lock
//! ```

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::version::Version;

/// Name of the symlink pointing at the active version.
pub const CURRENT: &str = "current";
/// Prefix of in-progress install directories.
pub const STAGING_PREFIX: &str = ".staging-";

/// Every path the base reads or writes, rooted at one directory so tests
/// can point it at a tempdir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    root: PathBuf,
}

impl Layout {
    /// A layout rooted at `root` (normally `$XDG_DATA_HOME/beagle`).
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// `$XDG_DATA_HOME/beagle`, falling back to `~/.local/share/beagle`.
    pub fn from_env() -> Result<Self> {
        beagle_utils::paths::xdg_data_home()
            .map(|d| Self::new(d.join("beagle")))
            .ok_or(Error::NoDataDir)
    }

    /// The layout root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// `extensions/`.
    pub fn extensions_dir(&self) -> PathBuf {
        self.root.join("extensions")
    }

    /// `extensions/<ext>/`.
    pub fn ext_dir(&self, ext: &str) -> PathBuf {
        self.extensions_dir().join(ext)
    }

    /// `extensions/<ext>/<version>/`.
    pub fn version_dir(&self, ext: &str, version: Version) -> PathBuf {
        self.ext_dir(ext).join(version.to_string())
    }

    /// `extensions/<ext>/current`.
    pub fn current_link(&self, ext: &str) -> PathBuf {
        self.ext_dir(ext).join(CURRENT)
    }

    /// `extensions/<ext>/.staging-<pid>/`.
    pub fn staging_dir(&self, ext: &str, pid: u32) -> PathBuf {
        self.ext_dir(ext).join(format!("{STAGING_PREFIX}{pid}"))
    }

    /// `locks/<name>.lock`.
    pub fn lock_file(&self, name: &str) -> PathBuf {
        self.root.join("locks").join(format!("{name}.lock"))
    }

    /// `state.toml`.
    pub fn state_file(&self) -> PathBuf {
        self.root.join("state.toml")
    }

    /// The active version of `ext`, read from its `current` link; `None`
    /// when not installed (or the link is dangling or malformed).
    pub fn current_version(&self, ext: &str) -> Option<Version> {
        let target = std::fs::read_link(self.current_link(ext)).ok()?;
        let version: Version = target.to_str()?.parse().ok()?;
        self.version_dir(ext, version).is_dir().then_some(version)
    }

    /// Every version of `ext` on disk, newest first.
    pub fn installed_versions(&self, ext: &str) -> Vec<Version> {
        let Ok(entries) = std::fs::read_dir(self.ext_dir(ext)) else {
            return Vec::new();
        };
        let mut versions: Vec<Version> = entries
            .filter_map(std::result::Result::ok)
            .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
            .filter_map(|e| e.file_name().to_str()?.parse().ok())
            .collect();
        versions.sort_unstable_by(|a, b| b.cmp(a));
        versions
    }

    /// The binary the base execs for `ext`: `current/bin/<bin>`.
    pub fn active_bin(&self, ext: &str, bin: &str) -> PathBuf {
        self.current_link(ext).join("bin").join(bin)
    }
}
