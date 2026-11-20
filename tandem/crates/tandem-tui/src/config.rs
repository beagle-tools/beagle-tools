//! Where the TUI keeps its data, and its optional config file.
//!
//! The store is its own SQLite file — not the desktop app's. The desktop
//! app drains `review_jobs` itself, so sharing a file would have it race
//! the agentd reviewer for every PR you queue.
//!
//! Both follow the one beagle folder (`docs/CODING_STANDARDS.md` rule 15):
//!
//! - Config, all optional: `$TANDEM_TUI_CONFIG` when set, else
//!   `~/.config/beagle/tandem.toml` (`$XDG_CONFIG_HOME` honoured), else the
//!   older `~/.config/tandem-tui/config.yaml`. A file that does not parse
//!   warns and the defaults apply.
//! - Store: `$BEAGLE_TANDEM_ROOT/cache.sqlite3`, else `$TANDEM_TUI_DB`, else
//!   `<root>/cache.sqlite3` for `root` in the config, else
//!   `<data dir>/tandem-tui/cache.sqlite3`.
//!
//! ```toml
//! # the gh account reads, posts and approvals use (`gh auth token -u`);
//! # omit to use gh's active account
//! github_user = "octocat"
//! # run after a PR is queued, so the review starts now rather than on
//! # the next scheduled tick; omit to only queue
//! kick = ["agentctl", "run", "demo/review-queue"]
//! ```

use std::path::{Path, PathBuf};

use beagle_tui::config::AppDirs;
use serde::Deserialize;
use tandem_core::{Result, TandemError};

/// The review TUI's names for the shared resolver.
pub(crate) const DIRS: AppDirs = AppDirs {
    app: "tandem",
    legacy_env: &[],
    legacy_configs: &[],
    marker: None,
    data_dir: None,
};

/// The store file's name inside a store root.
const DB_FILE: &str = "cache.sqlite3";

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub github_user: Option<String>,
    #[serde(default)]
    pub kick: Vec<String>,
    /// The directory holding the store (`cache.sqlite3`).
    #[serde(default)]
    pub root: Option<String>,
}

impl Config {
    /// The config file, or defaults when there is none. A file that does
    /// not parse warns on stderr (naming the file) and the defaults apply,
    /// so the TUI acts as gh's active account until it is fixed.
    #[must_use]
    pub fn load() -> Self {
        let Some(path) = config_path() else {
            return Self::default();
        };
        let (config, warning) = load_from(&path);
        if let Some(warning) = warning {
            eprintln!("beagle tandem: warning: {warning}");
        }
        config
    }
}

/// Reads `path` as TOML (`.toml`) or YAML (anything else, the older
/// format), giving the defaults and a warning when it can't be used.
pub(crate) fn load_from(path: &Path) -> (Config, Option<String>) {
    if path.extension().is_some_and(|e| e == "toml") {
        return beagle_tui::config::load_toml_or_default(path);
    }
    match std::fs::read_to_string(path) {
        Ok(text) => match serde_yaml::from_str(&text) {
            Ok(config) => (config, None),
            Err(e) => (
                Config::default(),
                Some(format!("{}: {e} (using defaults)", path.display())),
            ),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (Config::default(), None),
        Err(e) => (
            Config::default(),
            Some(format!("{}: {e} (using defaults)", path.display())),
        ),
    }
}

/// The config file in use: `$TANDEM_TUI_CONFIG`, else the first that
/// exists of `beagle/tandem.toml` and the older
/// `~/.config/tandem-tui/config.yaml`, else `beagle/tandem.toml`.
pub(crate) fn config_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("TANDEM_TUI_CONFIG") {
        return Some(PathBuf::from(p));
    }
    let config_home = beagle_utils::paths::xdg_config_home()?;
    let legacy = dirs::home_dir().map(|h| h.join(".config/tandem-tui/config.yaml"));
    Some(pick_config(&config_home, legacy))
}

/// `beagle/tandem.toml` under `config_home` when it exists, else `legacy`
/// when that exists, else `beagle/tandem.toml`.
pub(crate) fn pick_config(config_home: &Path, legacy: Option<PathBuf>) -> PathBuf {
    let new = DIRS.config_path_in(config_home);
    if new.is_file() {
        return new;
    }
    legacy.filter(|p| p.is_file()).unwrap_or(new)
}

/// The store: see the module docs for the order.
pub fn db_path() -> Result<PathBuf> {
    // Quietly: `Config::load` warns about a broken file once already.
    let config_root = config_path().and_then(|p| load_from(&p).0.root);
    db_path_from(
        DIRS.env_root().as_deref(),
        std::env::var_os("TANDEM_TUI_DB").map(PathBuf::from),
        config_root.as_deref(),
        dirs::data_dir(),
    )
}

/// The pure core of [`db_path`].
pub(crate) fn db_path_from(
    env_root: Option<&str>,
    legacy_db: Option<PathBuf>,
    config_root: Option<&str>,
    data_dir: Option<PathBuf>,
) -> Result<PathBuf> {
    if let Some(root) = env_root {
        return Ok(beagle_tui::config::expand_tilde(root).join(DB_FILE));
    }
    if let Some(db) = legacy_db.filter(|p| !p.as_os_str().is_empty()) {
        return Ok(db);
    }
    if let Some(root) = config_root {
        return Ok(beagle_tui::config::expand_tilde(root).join(DB_FILE));
    }
    data_dir
        .map(|d| d.join("tandem-tui").join(DB_FILE))
        .ok_or_else(|| TandemError::Config("no data directory for this user".into()))
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
