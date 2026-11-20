//! Where the TUI keeps its data, and its optional config file.
//!
//! The store is its own SQLite file — not the desktop app's. The desktop
//! app drains `review_jobs` itself, so sharing a file would have it race
//! the agentd reviewer for every PR you queue.
//!
//! Config, all optional (`~/.config/tandem-tui/config.yaml`, or
//! `$TANDEM_TUI_CONFIG`):
//!
//! ```yaml
//! # the gh account reads, posts and approvals use (`gh auth token -u`);
//! # omit to use gh's active account
//! github_user: octocat
//! # run after a PR is queued, so the review starts now rather than on
//! # the next scheduled tick; omit to only queue
//! kick: [agentctl, run, demo/review-queue]
//! ```

use std::path::PathBuf;

use serde::Deserialize;
use tandem_core::{Result, TandemError};

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub github_user: Option<String>,
    #[serde(default)]
    pub kick: Vec<String>,
}

impl Config {
    /// The config file, or defaults when there is none. A file that does
    /// not parse is an error: silently ignoring it would post as the
    /// wrong GitHub account.
    pub fn load() -> Result<Self> {
        let Some(path) = config_path() else {
            return Ok(Self::default());
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => serde_yaml::from_str(&text)
                .map_err(|e| TandemError::Config(format!("{}: {e}", path.display()))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }
}

pub(crate) fn config_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("TANDEM_TUI_CONFIG") {
        return Some(PathBuf::from(p));
    }
    Some(dirs::home_dir()?.join(".config/tandem-tui/config.yaml"))
}

/// The store: `$TANDEM_TUI_DB`, else `<data dir>/tandem-tui/cache.sqlite3`.
pub fn db_path() -> Result<PathBuf> {
    if let Some(p) = std::env::var_os("TANDEM_TUI_DB") {
        return Ok(PathBuf::from(p));
    }
    dirs::data_dir()
        .map(|d| d.join("tandem-tui").join("cache.sqlite3"))
        .ok_or_else(|| TandemError::Config("no data directory for this user".into()))
}
