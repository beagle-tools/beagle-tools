//! Settings commands, including GitHub owner/repo discovery for the
//! settings repo browser.

use serde::Serialize;
use tandem_core::TandemError;
use tandem_github::RepoSummary;
use tauri::State;

use crate::settings::Settings;
use crate::state::AppState;

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<Settings, TandemError> {
    Ok(state.settings.lock().await.clone())
}

#[tauri::command]
pub async fn update_settings(
    state: State<'_, AppState>,
    settings: Settings,
) -> Result<Settings, TandemError> {
    settings.save(&state.dirs.settings_file)?;
    let mut current = state.settings.lock().await;
    *current = settings;
    Ok(current.clone())
}

/// The authenticated user plus their orgs — owners the browser offers.
#[derive(Debug, Clone, Serialize)]
pub struct OwnerList {
    pub viewer: String,
    pub orgs: Vec<String>,
}

#[tauri::command]
pub async fn list_github_owners(state: State<'_, AppState>) -> Result<OwnerList, TandemError> {
    let client = state.github_client().await?;
    let viewer = client.viewer_login().await?;
    let orgs = client.list_orgs().await?;
    Ok(OwnerList { viewer, orgs })
}

#[tauri::command]
pub async fn list_github_repos(
    state: State<'_, AppState>,
    owner: String,
    is_viewer: bool,
) -> Result<Vec<RepoSummary>, TandemError> {
    let client = state.github_client().await?;
    client.list_owner_repos(&owner, is_viewer).await
}

#[tauri::command]
pub async fn file_config_info(
    state: State<'_, AppState>,
) -> Result<Option<crate::file_config::FileConfigInfo>, TandemError> {
    Ok(state.file_config.clone())
}

#[tauri::command]
pub async fn list_collaborators(
    state: State<'_, AppState>,
    repo: String,
) -> Result<Vec<String>, TandemError> {
    let repo = crate::commands::parse_repo(&repo)?;
    let client = state.github_client().await?;
    client.list_collaborators(&repo).await
}
