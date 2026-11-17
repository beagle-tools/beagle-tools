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
//! ```

use std::path::{Path, PathBuf};

use beagle_utils::paths::{env_path, expand_tilde, xdg_config_home};
use serde::Deserialize;

/// The parsed `ui.yaml`. A missing or unreadable file is the default:
/// no ticket links (beyond full URLs) and no investigations dir.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    /// Issue-tracker URL template; `{key}` is replaced with the ticket key.
    pub ticket_url: Option<String>,
    /// Directory holding `<run id>/OUTPUT.md` investigation write-ups.
    pub investigations_dir: Option<PathBuf>,
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
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_yaml::from_str(&text).ok())
            .unwrap_or_default()
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

    /// `investigations_dir` with `~` expanded.
    #[must_use]
    pub fn investigations_dir(&self) -> Option<PathBuf> {
        self.investigations_dir.as_deref().map(expand_tilde)
    }
}

/// `$AGENTD_CONFIG_DIR/ui.yaml`, else `$XDG_CONFIG_HOME/agentd/ui.yaml`
/// (`~/.config/agentd/ui.yaml`).
#[must_use]
pub fn config_path() -> Option<PathBuf> {
    env_path("AGENTD_CONFIG_DIR")
        .or_else(|| xdg_config_home().map(|d| d.join("agentd")))
        .map(|d| d.join("ui.yaml"))
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
            "hidden_projects: [demo]\nticket_url: https://t.example/{key}\ninvestigations_dir: /tmp/inv\n",
        )
        .unwrap();
        let cfg = UiConfig::load_from(&file);
        assert_eq!(cfg.ticket_url.as_deref(), Some("https://t.example/{key}"));
        assert_eq!(cfg.investigations_dir(), Some(PathBuf::from("/tmp/inv")));
        std::fs::write(&file, ": not yaml [").unwrap();
        assert_eq!(UiConfig::load_from(&file), UiConfig::default());
        assert_eq!(
            UiConfig::load_from(&dir.join("missing.yaml")),
            UiConfig::default()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
