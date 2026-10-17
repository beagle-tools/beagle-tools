//! `state.toml`: the cached result of the last release check per extension,
//! and when its "update available" notice was last shown.
//!
//! Dispatch only ever *reads* this cache (exec replaces the process, so the
//! notice must print before it, and the dispatch path never touches the
//! network). It is refreshed by `beagle extensions …` and `beagle update`.
//! A corrupt or missing file is treated as empty — it is only a cache.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::version::Version;

/// Minimum gap between two notices for one extension.
pub const NOTICE_INTERVAL_SECS: u64 = 24 * 60 * 60;

/// The whole cache.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    /// Per-extension entries, by canonical name.
    #[serde(default)]
    pub ext: BTreeMap<String, Entry>,
}

/// One extension's cache entry.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// Unix seconds of the last successful release check.
    #[serde(default)]
    pub checked_at: u64,
    /// The newest installable version seen then.
    #[serde(default)]
    pub latest: Option<Version>,
    /// Unix seconds the last notice was shown.
    #[serde(default)]
    pub notified_at: u64,
}

impl State {
    /// Reads `path`; missing or unreadable files give an empty state.
    pub fn load(path: &Path) -> Self {
        beagle_utils::fs::read_optional(path)
            .ok()
            .flatten()
            .and_then(|text| toml::from_str(&text).ok())
            .unwrap_or_default()
    }

    /// Writes the state atomically.
    pub fn save(&self, path: &Path) -> Result<()> {
        let text = toml::to_string(self).map_err(|e| Error::Manifest {
            path: path.to_path_buf(),
            message: e.to_string(),
        })?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
        }
        beagle_utils::fs::write_atomic(path, text.as_bytes()).map_err(|e| Error::io(path, e))
    }

    /// Records a release check for `name` at `now`.
    pub fn record_check(&mut self, name: &str, latest: Option<Version>, now: u64) {
        let entry = self.ext.entry(name.to_owned()).or_default();
        entry.checked_at = now;
        entry.latest = latest;
    }

    /// The newer version to announce for `name` running `active`, if the
    /// cache knows one and no notice was shown in the last 24 h.
    pub fn due_notice(&self, name: &str, active: Version, now: u64) -> Option<Version> {
        let entry = self.ext.get(name)?;
        let latest = entry.latest.filter(|l| *l > active)?;
        (now.saturating_sub(entry.notified_at) >= NOTICE_INTERVAL_SECS).then_some(latest)
    }

    /// Remembers that `name`'s notice was shown at `now`.
    pub fn mark_notified(&mut self, name: &str, now: u64) {
        self.ext.entry(name.to_owned()).or_default().notified_at = now;
    }
}

/// Whether notices are allowed by the environment: off when
/// `BEAGLE_NO_UPDATE_CHECK=1` or `CI` is set.
pub fn notices_enabled(no_check: Option<&str>, ci: Option<&str>) -> bool {
    no_check != Some("1") && ci.is_none()
}

/// Seconds since the Unix epoch.
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[cfg(test)]
#[path = "tests/state.rs"]
mod tests;
