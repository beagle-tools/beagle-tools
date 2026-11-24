//! Daemon-side settings for the git guard: `~/.config/agentd/agentd.yaml`.
//!
//! ```yaml
//! # Narrows the push guard to these GitHub orgs' repos. Empty (the
//! # default) guards main/master on EVERY remote.
//! protected_orgs: [acme]
//! ```
//!
//! `$AGENTD_PROTECTED_ORGS` (space- or comma-separated) in the DAEMON's
//! environment overrides the file. Every key is optional and unknown keys
//! are ignored, so a file written for a newer agentd still loads. agentd
//! bakes the list into its pre-push hook at startup; a run's own
//! environment never changes it.

use std::path::{Path, PathBuf};

use serde::Deserialize;

/// The parsed `agentd.yaml`. A missing or unreadable file is the default.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct DaemonConfig {
    /// Orgs the push guard is narrowed to; empty guards every remote.
    pub protected_orgs: Vec<String>,
}

impl DaemonConfig {
    /// Load from [`config_path`], falling back to the default on any error.
    #[must_use]
    pub fn load() -> Self {
        config_path().map_or_else(Self::default, |p| Self::load_from(&p))
    }

    /// Load from an explicit path (see [`DaemonConfig::load`]).
    #[must_use]
    pub fn load_from(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_yaml::from_str(&text).ok())
            .unwrap_or_default()
    }
}

/// `agentd.yaml` inside [`crate::paths::config_dir`].
#[must_use]
pub fn config_path() -> Option<PathBuf> {
    crate::paths::config_dir()
        .ok()
        .map(|d| d.join("agentd.yaml"))
}

/// The protected orgs: `$AGENTD_PROTECTED_ORGS` when set, else the config
/// file's `protected_orgs`, else none (which guards every remote).
#[must_use]
pub fn protected_orgs() -> Vec<String> {
    match std::env::var("AGENTD_PROTECTED_ORGS") {
        Ok(raw) => parse_list(&raw),
        Err(_) => DaemonConfig::load().protected_orgs,
    }
}

/// Split a space- or comma-separated org list, dropping empty entries.
#[must_use]
pub fn parse_list(raw: &str) -> Vec<String> {
    raw.split(|c: char| c == ',' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

#[cfg(test)]
// Tests should fail loudly on a broken fixture; unwrapping is the clearest way.
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn lists_split_on_spaces_and_commas() {
        assert_eq!(parse_list(" acme, widgets  co "), ["acme", "widgets", "co"]);
        assert_eq!(parse_list("  "), Vec::<String>::new());
    }

    #[test]
    fn a_missing_or_bad_file_protects_nothing() {
        let dir = std::env::temp_dir().join(format!("agent-core-guard-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("agentd.yaml");
        std::fs::write(&file, "future_key: 1\nprotected_orgs: [acme]\n").unwrap();
        assert_eq!(DaemonConfig::load_from(&file).protected_orgs, ["acme"]);
        std::fs::write(&file, ": not yaml [").unwrap();
        assert_eq!(DaemonConfig::load_from(&file), DaemonConfig::default());
        assert_eq!(
            DaemonConfig::load_from(&dir.join("missing.yaml")),
            DaemonConfig::default()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
