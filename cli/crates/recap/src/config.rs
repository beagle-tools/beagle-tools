//! Where the recaps live — the store root, the directory containing
//! `recaps/` — and the rest of `~/.config/recap/config.toml`.
//!
//! Resolution order: `--root` flag → `$RECAP_ROOT` → `root` in
//! `~/.config/recap/config.toml` → the nearest ancestor of the working
//! directory containing `recaps/` → the working directory. A leading `~/`
//! is expanded everywhere. The rules live in [`beagle_tui::config`].

use std::path::{Path, PathBuf};

pub use beagle_tui::config::expand_tilde;
use beagle_tui::config::AppDirs;
use serde::Deserialize;

use crate::error::Result;
use crate::store::RECAPS_DIR;

/// The app's names: config dir, env var and data dir.
pub const DIRS: AppDirs = AppDirs {
    app: "recap",
    env_var: "RECAP_ROOT",
    data_dir: RECAPS_DIR,
};

/// `~/.config/recap/config.toml`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Default store root.
    pub root: Option<String>,
    /// `[comments]`: whose comments the Comments tab and the review agent
    /// ignore, and which comments are automated replies.
    #[serde(default)]
    pub comments: crate::comments::CommentsConfig,
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

/// The config file's contents, or the defaults when there is none or it
/// can't be read (callers that must report a broken file use [`load`]).
#[must_use]
pub fn load_or_default() -> Config {
    config_path()
        .and_then(|p| load(&p).ok())
        .unwrap_or_default()
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
