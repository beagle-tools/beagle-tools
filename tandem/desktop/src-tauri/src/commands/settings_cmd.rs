//! Settings commands, including GitHub owner/repo discovery for the
//! settings repo browser.

use serde::Serialize;
use tandem_core::TandemError;
use tandem_github::{GithubAuth, RepoSummary};
use tauri::State;

use crate::settings::Settings;
use crate::state::AppState;

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<Settings, TandemError> {
    Ok(masked(state.settings.lock().await.clone()))
}

#[tauri::command]
pub async fn update_settings(
    state: State<'_, AppState>,
    mut settings: Settings,
) -> Result<Settings, TandemError> {
    let mut current = state.settings.lock().await;
    keep_token_if_masked(&mut settings, &current);
    settings.save(&state.dirs.settings_file)?;
    *current = settings;
    Ok(masked(current.clone()))
}

/// What the webview sees in place of a PAT: enough to recognise it.
fn mask(token: &str) -> String {
    let tail: String = token
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("•••• {tail}")
}

/// Settings with the PAT replaced by its mask. The real token never
/// crosses into the webview.
fn masked(mut settings: Settings) -> Settings {
    if let GithubAuth::Token { token } = &mut settings.github.auth {
        *token = mask(token);
    }
    settings
}

/// A save that sends the mask back unchanged keeps the stored token.
fn keep_token_if_masked(incoming: &mut Settings, current: &Settings) {
    if let (GithubAuth::Token { token: new }, GithubAuth::Token { token: old }) =
        (&mut incoming.github.auth, &current.github.auth)
    {
        if *new == mask(old) {
            new.clone_from(old);
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn with_token(token: &str) -> Settings {
        let mut s = Settings::default();
        s.github.auth = GithubAuth::Token {
            token: token.to_owned(),
        };
        s
    }

    #[test]
    fn the_webview_gets_a_mask_and_an_unchanged_save_keeps_the_token() {
        let real = with_token("ghp_abcdefghij1234");
        let shown = masked(real.clone());
        assert_eq!(shown.github.auth, with_token("•••• 1234").github.auth);
        let mut saved = shown;
        keep_token_if_masked(&mut saved, &real);
        assert_eq!(saved.github.auth, real.github.auth);
        // A new token pasted in replaces it.
        let mut replaced = with_token("ghp_new");
        keep_token_if_masked(&mut replaced, &real);
        assert_eq!(replaced.github.auth, with_token("ghp_new").github.auth);
    }
}
