//! User settings, persisted as JSON in the platform config directory.
//!
//! NOTE: a PAT configured via `GithubAuth::Token` currently lives in this
//! file (created with 0600 perms). Moving secrets to the OS keychain is
//! on the roadmap (docs/ROADMAP.md).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tandem_core::{Result, TandemError};
use tandem_github::GithubConfig;

/// Bump when a migration in [`Settings::load`] needs to run once.
const SETTINGS_VERSION: u32 = 3;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub github: GithubConfig,
    /// Owners (orgs, or your own GitHub login) tracked for the repo
    /// picker. The repo list itself is fetched live from GitHub, never
    /// stored here.
    pub orgs: Vec<String>,
    /// Repo shown on a fresh, uncached launch (`owner/name`); `None`
    /// falls back to the repo picker.
    pub default_repo: Option<String>,
    /// Default PR-list filters, applied whenever a repo is opened; the
    /// user can adjust or clear them at runtime without saving.
    pub pr_filters: PrFilters,
    /// Default ordering for PR lists ("opened-asc", "updated-desc", ...).
    #[serde(default = "default_pr_sort")]
    pub pr_sort: String,
    /// Whether inbox tabs search every repo instead of the selected one.
    pub inbox_all_repos: bool,
    /// How often (minutes) the review-status lists refresh themselves in
    /// the background. 0 turns the background refresh off — these are
    /// GitHub search calls, so the knob is here to stay under the rate
    /// limit on big accounts.
    #[serde(default = "default_refresh_minutes")]
    pub refresh_minutes: u32,
    /// Command "open in editor" runs on a working tree — e.g. "code",
    /// "cursor", "zed", "subl". Terminal editors need a wrapper.
    #[serde(default = "default_editor_command")]
    pub editor_command: String,
    /// Known local checkouts, repo slug -> absolute path. Used before
    /// falling back to a Tandem-managed clone (which is never the case
    /// for these paths: user checkouts are never mutated).
    #[serde(default)]
    pub repo_paths: std::collections::BTreeMap<String, String>,
    /// Settings schema version (for one-time migrations on load).
    pub version: u32,
}

fn default_pr_sort() -> String {
    "opened-desc".to_owned()
}

fn default_editor_command() -> String {
    "code".to_owned()
}

fn default_refresh_minutes() -> u32 {
    5
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            github: GithubConfig::default(),
            orgs: Vec::new(),
            default_repo: None,
            pr_filters: PrFilters::default(),
            pr_sort: default_pr_sort(),
            inbox_all_repos: false,
            refresh_minutes: default_refresh_minutes(),
            editor_command: default_editor_command(),
            repo_paths: std::collections::BTreeMap::new(),
            version: SETTINGS_VERSION,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PrFilters {
    pub query: String,
    pub author: String,
    pub label: String,
    #[serde(default = "default_true")]
    pub hide_drafts: bool,
}

fn default_true() -> bool {
    true
}

impl Default for PrFilters {
    fn default() -> Self {
        Self {
            query: String::new(),
            author: String::new(),
            label: String::new(),
            // Drafts are noise for most review flows — hide by default.
            hide_drafts: true,
        }
    }
}

/// Where Tandem keeps its files on disk.
#[derive(Debug, Clone)]
pub struct AppDirs {
    pub settings_file: PathBuf,
    pub cache_db: PathBuf,
    pub runs_dir: PathBuf,
    pub worktrees_dir: PathBuf,
}

// `cargo tauri dev` is always a debug build, so this separates a dev
// run's settings/cache/runs from the installed release app's — no more
// `pnpm tauri dev` quietly reading and rewriting your real PR review
// notes and settings.
const APP_DIR_NAME: &str = if cfg!(debug_assertions) {
    "tandem-dev"
} else {
    "tandem"
};

impl AppDirs {
    pub fn resolve() -> Result<Self> {
        let config_root = dirs::config_dir()
            .ok_or_else(|| TandemError::Config("no config directory on this platform".into()))?;
        let data_root = dirs::data_dir()
            .ok_or_else(|| TandemError::Config("no data directory on this platform".into()))?;
        // The app used to be called "appa" — carry existing data across.
        // Only the production dir ever had that name; a dev build never
        // did, so this migration doesn't apply to it.
        migrate_legacy_dir(&config_root.join("appa"), &config_root.join("tandem"));
        migrate_legacy_dir(&data_root.join("appa"), &data_root.join("tandem"));

        let config = config_root.join(APP_DIR_NAME);
        let data = data_root.join(APP_DIR_NAME);
        Ok(Self {
            settings_file: config.join("settings.json"),
            cache_db: data.join("cache.sqlite3"),
            runs_dir: data.join("runs"),
            worktrees_dir: data.join("worktrees"),
        })
    }
}

/// Best-effort one-time rename of a pre-rename data directory.
fn migrate_legacy_dir(old: &Path, new: &Path) {
    if old.is_dir() && !new.exists() {
        match std::fs::rename(old, new) {
            Ok(()) => {
                tracing::info!(from = %old.display(), to = %new.display(), "migrated data dir");
            }
            Err(e) => tracing::warn!(error = %e, "failed to migrate legacy data dir"),
        }
    }
}

impl Settings {
    pub fn load(path: &Path) -> Result<Self> {
        match beagle_utils::fs::read_optional(path)? {
            Some(contents) => {
                let raw: serde_json::Value = serde_json::from_str(&contents)?;
                // Captured before typed deserialization drops it: `repos`
                // no longer exists on `Settings` as of v3.
                let legacy_repos: Vec<String> = raw
                    .get("repos")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(str::to_owned))
                            .collect()
                    })
                    .unwrap_or_default();
                let mut settings: Self = serde_json::from_value(raw)?;
                // v0 -> v1: files saved before the filters UI existed may
                // carry an unintended hide_drafts=false — restore the
                // intended default exactly once.
                if settings.version < 1 {
                    settings.pr_filters.hide_drafts = true;
                }
                // v1 -> v2: the default ordering changed to newest-opened;
                // carry files that still hold the old default across.
                if settings.version < 2 && settings.pr_sort == "opened-asc" {
                    "opened-desc".clone_into(&mut settings.pr_sort);
                }
                // v2 -> v3: the flat tracked-repo list (`repos`) was
                // replaced by tracked owners (`orgs`) + an explicit
                // `default_repo`. Derive both from the old list so
                // existing setups don't need to reconfigure anything.
                if settings.version < 3 {
                    for slug in &legacy_repos {
                        if let Some(owner) = slug.split('/').next() {
                            if !settings.orgs.iter().any(|o| o == owner) {
                                settings.orgs.push(owner.to_owned());
                            }
                        }
                    }
                    if settings.default_repo.is_none() {
                        settings.default_repo = legacy_repos.first().cloned();
                    }
                }
                if settings.version < SETTINGS_VERSION {
                    settings.version = SETTINGS_VERSION;
                }
                Ok(settings)
            }
            None => Ok(Self::default()),
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        // Atomic, and 0600 *before* the file becomes visible: the file can
        // hold a PAT, so it must never exist with looser permissions, and a
        // crash mid-save must not leave a truncated settings file.
        beagle_utils::fs::write_atomic_with_mode(path, json.as_bytes(), 0o600)?;
        Ok(())
    }
}

#[cfg(test)]
// Tests fail loudly on setup errors; unwrap is the clearest way to do that.
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips_settings() {
        let dir = std::env::temp_dir().join(format!("tandem-settings-{}", std::process::id()));
        let path = dir.join("settings.json");
        let mut s = Settings::default();
        s.orgs.push("matthewmyrick".into());
        s.default_repo = Some("matthewmyrick/code-review".into());
        s.save(&path).unwrap();
        let loaded = Settings::load(&path).unwrap();
        assert_eq!(loaded.orgs, s.orgs);
        assert_eq!(loaded.default_repo, s.default_repo);
        std::fs::remove_dir_all(dir).ok();
    }

    #[cfg(unix)]
    #[test]
    fn saved_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = std::env::temp_dir().join(format!("tandem-settings-mode-{}", std::process::id()));
        let path = dir.join("settings.json");
        Settings::default().save(&path).unwrap();
        // Saving over an existing file keeps it 0600 too.
        Settings::default().save(&path).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn missing_file_yields_defaults() {
        let loaded = Settings::load(Path::new("/nonexistent/tandem/settings.json")).unwrap();
        assert_eq!(loaded.orgs, [] as [std::string::String; 0]);
        assert!(loaded.default_repo.is_none());
    }

    #[test]
    fn migrates_legacy_repos_to_orgs_and_default_repo() {
        let dir =
            std::env::temp_dir().join(format!("tandem-settings-migrate-{}", std::process::id()));
        let path = dir.join("settings.json");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            &path,
            r#"{"version":2,"repos":["matthewmyrick/code-review","matthewmyrick/dotfiles","other-org/thing"]}"#,
        )
        .unwrap();
        let loaded = Settings::load(&path).unwrap();
        assert_eq!(loaded.orgs, vec!["matthewmyrick", "other-org"]);
        assert_eq!(
            loaded.default_repo,
            Some("matthewmyrick/code-review".to_owned())
        );
        assert_eq!(loaded.version, SETTINGS_VERSION);
        std::fs::remove_dir_all(dir).ok();
    }
}
