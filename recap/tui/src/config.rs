//! Where the recaps live — the store root, the directory containing
//! `recaps/` — and the rest of the config file.
//!
//! Config: `~/.config/beagle/recap.toml` (`$XDG_CONFIG_HOME` honoured), else
//! the older `~/.config/recap/config.toml`. A broken file warns and the
//! defaults apply.
//!
//! Store root: `--root` flag → `$BEAGLE_RECAP_ROOT` → `$RECAP_ROOT` →
//! `root` in the config → the nearest ancestor of the working directory
//! containing `recaps/` → the working directory. A leading `~/` is expanded
//! everywhere. The rules live in [`beagle_tui::config`].

use std::path::{Path, PathBuf};

pub use beagle_tui::config::expand_tilde;
use beagle_tui::config::AppDirs;
use serde::Deserialize;

use crate::error::Result;
use crate::store::RECAPS_DIR;

/// The tool's names: config file, env vars and data dir.
pub const DIRS: AppDirs = AppDirs {
    app: "recap",
    legacy_env: &["RECAP_ROOT"],
    legacy_configs: &["recap/config.toml"],
    marker: None,
    data_dir: Some(RECAPS_DIR),
};

/// The config file (`~/.config/beagle/recap.toml`).
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

/// The config file in use: `beagle/recap.toml`, else the older
/// `recap/config.toml` when only that exists.
#[must_use]
pub fn config_path() -> Option<PathBuf> {
    DIRS.config_file()
}

/// Loads the config file; a missing file is the default config.
///
/// # Errors
/// [`crate::Error::Config`] for an unreadable or invalid file.
pub fn load(path: &Path) -> Result<Config> {
    Ok(beagle_tui::config::load_toml(path)?)
}

/// The config file's contents, or the defaults when there is none or it
/// can't be used (with a warning on stderr then).
#[must_use]
pub fn load_or_default() -> Config {
    let (config, warning) = DIRS.load_config();
    if let Some(warning) = warning {
        eprintln!("beagle recap: warning: {warning}");
    }
    config
}

/// [`load_or_default`] without the warning, for background threads running
/// under the TUI (stderr would draw over it).
#[must_use]
pub fn load_quietly() -> Config {
    DIRS.load_config().0
}

/// Picks the store root from the flag, environment, and config, falling
/// back to discovery from `cwd` (see the module docs for the order).
#[must_use]
pub fn resolve_root(flag: Option<&str>, env: Option<&str>, config: &Config, cwd: &Path) -> PathBuf {
    DIRS.resolve_root(flag, env, config.root.as_deref(), cwd)
}

/// The store root `beagle recap` uses: `flag` (`--root`) when given, else
/// the environment, the config file and discovery from the working
/// directory, in the order the module docs give. `beagle serve` calls it
/// with no flag, so the phone reads the store the TUI shows.
///
/// # Errors
/// An unreadable working directory.
pub fn root_for(flag: Option<&str>) -> Result<PathBuf> {
    let config = load_or_default();
    let env_root = DIRS.env_root();
    let cwd = std::env::current_dir().map_err(|e| crate::Error::io(".", e))?;
    Ok(resolve_root(flag, env_root.as_deref(), &config, &cwd))
}

#[cfg(test)]
#[path = "tests/config.rs"]
mod tests;
