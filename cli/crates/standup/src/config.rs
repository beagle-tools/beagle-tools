//! Where the reports live — the store root, the directory containing
//! `standups/`.
//!
//! Config: `~/.config/beagle/standup.toml` (`$XDG_CONFIG_HOME` honoured),
//! else the older `~/.config/standup/config.toml`. A broken file warns and
//! the defaults apply.
//!
//! Store root: `--root` flag → `$BEAGLE_STANDUP_ROOT` → `$STANDUP_ROOT` →
//! `root` in the config → the nearest ancestor of the working directory
//! containing `standups/` → the working directory. A leading `~/` is
//! expanded everywhere. The rules live in [`beagle_tui::config`].

use std::path::{Path, PathBuf};

pub use beagle_tui::config::expand_tilde;
use beagle_tui::config::AppDirs;
use serde::Deserialize;

use crate::error::Result;
use crate::store::REPORTS_DIR;

/// The tool's names: config file, env vars and data dir.
pub const DIRS: AppDirs = AppDirs {
    app: "standup",
    legacy_env: &["STANDUP_ROOT"],
    legacy_configs: &["standup/config.toml"],
    marker: None,
    data_dir: Some(REPORTS_DIR),
};

/// The config file (`~/.config/beagle/standup.toml`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Default store root.
    pub root: Option<String>,
}

/// The config file in use: `beagle/standup.toml`, else the older
/// `standup/config.toml` when only that exists.
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

/// Picks the store root from the flag, environment, and config, falling
/// back to discovery from `cwd` (see the module docs for the order).
#[must_use]
pub fn resolve_root(flag: Option<&str>, env: Option<&str>, config: &Config, cwd: &Path) -> PathBuf {
    DIRS.resolve_root(flag, env, config.root.as_deref(), cwd)
}

/// The store root `beagle standup` uses: `flag` (`--root`) when given, else
/// the environment, the config file and discovery from the working
/// directory, in the order the module docs give. `beagle serve` calls it
/// with no flag, so the phone reads the store the TUI shows. A broken
/// config warns on stderr and the defaults apply.
///
/// # Errors
/// An unreadable working directory.
pub fn root_for(flag: Option<&str>) -> Result<PathBuf> {
    let (config, warning) = DIRS.load_config::<Config>();
    if let Some(warning) = warning {
        eprintln!("beagle standup: warning: {warning}");
    }
    let env_root = DIRS.env_root();
    let cwd = std::env::current_dir().map_err(|e| crate::Error::io(".", e))?;
    Ok(resolve_root(flag, env_root.as_deref(), &config, &cwd))
}

#[cfg(test)]
#[path = "tests/config.rs"]
mod tests;
