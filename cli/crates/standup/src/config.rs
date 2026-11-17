//! Where the reports live — the store root, the directory containing
//! `standups/`.
//!
//! Resolution order: `--root` flag → `$STANDUP_ROOT` → `root` in
//! `~/.config/standup/config.toml` → the nearest ancestor of the working
//! directory containing `standups/` → the working directory. A leading `~/`
//! is expanded everywhere. The rules live in [`beagle_tui::config`].

use std::path::{Path, PathBuf};

pub use beagle_tui::config::expand_tilde;
use beagle_tui::config::AppDirs;
use serde::Deserialize;

use crate::error::Result;
use crate::store::REPORTS_DIR;

/// The app's names: config dir, env var and data dir.
pub const DIRS: AppDirs = AppDirs {
    app: "standup",
    env_var: "STANDUP_ROOT",
    data_dir: REPORTS_DIR,
};

/// `~/.config/standup/config.toml`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Default store root.
    pub root: Option<String>,
}

/// The config file path (`$XDG_CONFIG_HOME` or `~/.config`).
#[must_use]
pub fn config_path() -> Option<PathBuf> {
    DIRS.config_path()
}

/// Loads the config file; a missing file is the default config.
///
/// # Errors
/// [`crate::Error::Config`] for an unreadable or invalid file.
pub fn load(path: &Path) -> Result<Config> {
    Ok(beagle_tui::config::load_toml(path)?)
}

/// Picks the store root from the flag, environment, and config, falling
/// back to discovery from `cwd` (see the module docs for the order).
#[must_use]
pub fn resolve_root(flag: Option<&str>, env: Option<&str>, config: &Config, cwd: &Path) -> PathBuf {
    DIRS.resolve_root(flag, env, config.root.as_deref(), cwd)
}

#[cfg(test)]
#[path = "tests/config.rs"]
mod tests;
