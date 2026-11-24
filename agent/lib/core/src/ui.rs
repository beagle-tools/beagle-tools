//! Per-user display settings: `~/.config/agentd/ui.yaml`.
//!
//! Nothing here changes what the daemon does; it only tells agenttop and
//! Agentdesk how to turn what a run carries into something you can open.
//! Every key is optional and unknown keys are ignored, so a file written
//! for a newer agentd still loads here.
//!
//! ```yaml
//! # Where a `ticket=<KEY>` param points. `{key}` is replaced.
//! ticket_url: https://tracker.example.com/issue/{key}
//! # Where long-lived investigation write-ups land: <dir>/<run id>/OUTPUT.md
//! investigations_dir: ~/notes/investigations
//! # Projects left out of the jobs screens. Their jobs still exist and
//! # still run (`agentctl run demo/writeup`); they are only not shown.
//! hidden_projects: [demo]
//! ```
//!
//! A viewing preference, not desired state: it lives beside the manifests
//! root, not in any manifest, so the same manifests can be watched with a
//! different set hidden on another machine. Read on every refresh, so an
//! edit shows within a tick.

use std::path::{Path, PathBuf};

use beagle_utils::paths::expand_tilde;
use serde::Deserialize;

/// The parsed `ui.yaml`. A missing or unreadable file is the default:
/// no ticket links (beyond full URLs), no investigations dir, nothing hidden.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    /// Issue-tracker URL template; `{key}` is replaced with the ticket key.
    pub ticket_url: Option<String>,
    /// Directory holding `<run id>/OUTPUT.md` investigation write-ups.
    pub investigations_dir: Option<PathBuf>,
    /// Projects whose jobs the jobs screens leave out.
    pub hidden_projects: Vec<String>,
}

impl UiConfig {
    /// Load from [`config_path`], falling back to the default on any error:
    /// these are conveniences, and a broken file must not break the UI.
    #[must_use]
    pub fn load() -> Self {
        config_path().map_or_else(Self::default, |p| Self::load_from(&p))
    }

    /// Load from an explicit path (see [`UiConfig::load`]).
    #[must_use]
    pub fn load_from(path: &Path) -> Self {
        Self::try_load_from(path).unwrap_or_default()
    }

    /// Like [`UiConfig::load`], but a file that exists and does not parse
    /// is an error, so a UI can say so instead of silently showing
    /// everything. A missing file is still the default.
    pub fn try_load() -> Result<Self, String> {
        config_path().map_or_else(|| Ok(Self::default()), |p| Self::try_load_from(&p))
    }

    /// [`UiConfig::try_load`] from an explicit path.
    pub fn try_load_from(path: &Path) -> Result<Self, String> {
        match std::fs::read_to_string(path) {
            Ok(text) if text.trim().is_empty() => Ok(Self::default()),
            Ok(text) => serde_yaml::from_str(&text).map_err(|e| e.to_string()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    /// The URL for a run's `ticket` param: a full URL as given, otherwise
    /// the key expanded through `ticket_url`. `None` when there is no
    /// template to expand a bare key with.
    #[must_use]
    pub fn ticket_url(&self, raw: &str) -> Option<String> {
        let raw = raw.trim();
        if raw.is_empty() {
            return None;
        }
        if raw.starts_with("https://") || raw.starts_with("http://") {
            return Some(raw.to_owned());
        }
        let template = self.ticket_url.as_deref()?;
        Some(template.replace("{key}", raw))
    }

    /// Whether the jobs screens leave `project` out.
    #[must_use]
    pub fn hides(&self, project: &str) -> bool {
        self.hidden_projects.iter().any(|p| p == project)
    }

    /// `investigations_dir` with `~` expanded.
    #[must_use]
    pub fn investigations_dir(&self) -> Option<PathBuf> {
        self.investigations_dir.as_deref().map(expand_tilde)
    }
}

/// `ui.yaml` inside [`crate::paths::config_dir`]
/// (`$AGENTD_CONFIG_DIR`, else `~/.config/agentd`).
#[must_use]
pub fn config_path() -> Option<PathBuf> {
    crate::paths::config_dir().ok().map(|d| d.join("ui.yaml"))
}

/// The key part of a `ticket` param, which may be a key or a pasted
/// tracker URL (`…/issue/<KEY>/<slug>`).
#[must_use]
pub fn ticket_key(raw: &str) -> String {
    raw.split_once("/issue/")
        .and_then(|(_, rest)| rest.split('/').next())
        .filter(|k| !k.is_empty())
        .unwrap_or(raw)
        .to_owned()
}

#[cfg(test)]
// Tests should fail loudly on a broken fixture; unwrapping is the clearest way.
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn with_template() -> UiConfig {
        UiConfig {
            ticket_url: Some("https://tracker.example.com/issue/{key}".to_owned()),
            investigations_dir: None,
            hidden_projects: Vec::new(),
        }
    }

    #[test]
    fn a_bare_key_needs_a_template() {
        assert_eq!(UiConfig::default().ticket_url("ABC-1"), None);
        assert_eq!(
            with_template().ticket_url(" ABC-1 ").as_deref(),
            Some("https://tracker.example.com/issue/ABC-1")
        );
    }

    #[test]
    fn a_full_url_is_used_as_given() {
        let url = "https://tracker.example.com/issue/ABC-1/some-slug";
        assert_eq!(UiConfig::default().ticket_url(url).as_deref(), Some(url));
        assert_eq!(UiConfig::default().ticket_url("  "), None);
    }

    #[test]
    fn ticket_key_accepts_a_key_or_a_url() {
        assert_eq!(ticket_key("ABC-1"), "ABC-1");
        assert_eq!(
            ticket_key("https://tracker.example.com/issue/ABC-1/some-slug"),
            "ABC-1"
        );
    }

    #[test]
    fn unknown_keys_and_bad_files_fall_back_to_defaults() {
        let dir = std::env::temp_dir().join(format!("agent-core-ui-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("ui.yaml");
        std::fs::write(
            &file,
            "future_key: 1\nhidden_projects: [demo]\nticket_url: https://t.example/{key}\ninvestigations_dir: /tmp/inv\n",
        )
        .unwrap();
        let cfg = UiConfig::load_from(&file);
        assert_eq!(cfg.ticket_url.as_deref(), Some("https://t.example/{key}"));
        assert_eq!(cfg.investigations_dir(), Some(PathBuf::from("/tmp/inv")));
        assert!(cfg.hides("demo"));
        assert!(!cfg.hides("widgets"));
        std::fs::write(&file, ": not yaml [").unwrap();
        assert_eq!(UiConfig::load_from(&file), UiConfig::default());
        assert!(UiConfig::try_load_from(&file).is_err());
        std::fs::write(&file, "  \n").unwrap();
        assert_eq!(UiConfig::try_load_from(&file), Ok(UiConfig::default()));
        assert_eq!(
            UiConfig::load_from(&dir.join("missing.yaml")),
            UiConfig::default()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
