//! OPTIONAL declarative config for dotfiles users. If a YAML file exists
//! it is loaded at startup: agents are upserted by name and any settings
//! fields present override the GUI-saved values in memory (the GUI
//! settings file is not rewritten). No file, no behavior change.
//!
//! Lookup order: `$TANDEM_CONFIG`, `~/.config/tandem/tandem.yaml`, then
//! `<platform config dir>/tandem/tandem.yaml`.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tandem_cache::{Cache, ReviewStore};
use tandem_core::agent::AgentSpec;

use crate::settings::{PrFilters, Settings};

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FileConfig {
    pub orgs: Option<Vec<String>>,
    pub default_repo: Option<String>,
    pub pr_filters: Option<PrFilters>,
    pub pr_sort: Option<String>,
    pub inbox_all_repos: Option<bool>,
    pub refresh_minutes: Option<u32>,
    pub agents: Option<Vec<AgentSpec>>,
}

/// What the UI shows when a file config is active.
#[derive(Debug, Clone, Serialize)]
pub struct FileConfigInfo {
    pub path: String,
    pub agents: usize,
    pub overrides: Vec<String>,
}

fn candidate_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(explicit) = std::env::var("TANDEM_CONFIG") {
        if !explicit.trim().is_empty() {
            paths.push(PathBuf::from(explicit));
        }
    }
    if let Some(home) = dirs::home_dir() {
        paths.push(home.join(".config/tandem/tandem.yaml"));
    }
    if let Some(config) = dirs::config_dir() {
        paths.push(config.join("tandem/tandem.yaml"));
    }
    paths
}

/// Load the first config file that exists; apply it to the in-memory
/// settings and upsert declared agents. Errors are logged, never fatal —
/// a broken optional config must not brick the app.
pub fn load_and_apply(settings: &mut Settings, cache: &Cache) -> Option<FileConfigInfo> {
    let path = candidate_paths().into_iter().find(|p| p.is_file())?;
    let contents = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "failed to read file config");
            return None;
        }
    };
    let config: FileConfig = match serde_yaml::from_str(&contents) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "invalid file config — ignoring");
            return None;
        }
    };

    let mut overrides = Vec::new();
    if let Some(orgs) = &config.orgs {
        for org in orgs {
            if !settings.orgs.contains(org) {
                settings.orgs.push(org.clone());
            }
        }
        overrides.push("orgs".to_owned());
    }
    if let Some(default_repo) = &config.default_repo {
        settings.default_repo = Some(default_repo.clone());
        overrides.push("default_repo".to_owned());
    }
    if let Some(filters) = &config.pr_filters {
        settings.pr_filters = filters.clone();
        overrides.push("pr_filters".to_owned());
    }
    if let Some(sort) = &config.pr_sort {
        settings.pr_sort.clone_from(sort);
        overrides.push("pr_sort".to_owned());
    }
    if let Some(all) = config.inbox_all_repos {
        settings.inbox_all_repos = all;
        overrides.push("inbox_all_repos".to_owned());
    }
    if let Some(minutes) = config.refresh_minutes {
        settings.refresh_minutes = minutes;
        overrides.push("refresh_minutes".to_owned());
    }

    let mut agent_count = 0;
    if let Some(agents) = &config.agents {
        for spec in agents {
            if spec.name.trim().is_empty() {
                tracing::warn!("file config agent with empty name skipped");
                continue;
            }
            match cache.put_agent_spec(spec) {
                Ok(()) => agent_count += 1,
                Err(e) => tracing::warn!(agent = %spec.name, error = %e, "failed to upsert agent"),
            }
        }
    }

    tracing::info!(path = %path.display(), agents = agent_count, "applied optional file config");
    Some(FileConfigInfo {
        path: path.display().to_string(),
        agents: agent_count,
        overrides,
    })
}
