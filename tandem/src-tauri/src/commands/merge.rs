//! Merge controls for the viewer's OWN pull requests — merge now,
//! update-from-base, and auto-merge. All explicit user actions; the UI
//! additionally hides these unless the PR author is the signed-in user.

use tandem_core::TandemError;
use tandem_github::MergeOptions;
use tauri::State;

use crate::commands::parse_repo;
use crate::state::AppState;

fn validate_method(method: &str) -> Result<(), TandemError> {
    match method {
        "merge" | "squash" | "rebase" => Ok(()),
        other => Err(TandemError::Config(format!(
            "unknown merge method: {other}"
        ))),
    }
}

#[tauri::command]
pub async fn merge_pr(
    state: State<'_, AppState>,
    repo: String,
    number: u64,
    method: String,
) -> Result<(), TandemError> {
    validate_method(&method)?;
    let repo = parse_repo(&repo)?;
    let client = state.github_client().await?;
    client.merge_pull_request(&repo, number, &method).await
}

#[tauri::command]
pub async fn update_pr_branch(
    state: State<'_, AppState>,
    repo: String,
    number: u64,
) -> Result<(), TandemError> {
    let repo = parse_repo(&repo)?;
    let client = state.github_client().await?;
    client.update_branch(&repo, number).await
}

#[tauri::command]
pub async fn enable_auto_merge(
    state: State<'_, AppState>,
    repo: String,
    number: u64,
    method: String,
) -> Result<(), TandemError> {
    validate_method(&method)?;
    let repo = parse_repo(&repo)?;
    let client = state.github_client().await?;
    // Fetch fresh to guarantee a node id (older cached PRs lack it).
    let pr = client.pull_request(&repo, number).await?;
    let node_id = pr.node_id.ok_or_else(|| TandemError::GithubApi {
        status: 0,
        message: "GitHub did not return a node id for this PR".into(),
    })?;
    client.enable_auto_merge(&node_id, &method).await
}

#[tauri::command]
pub async fn repo_merge_options(
    state: State<'_, AppState>,
    repo: String,
) -> Result<MergeOptions, TandemError> {
    let repo = parse_repo(&repo)?;
    let client = state.github_client().await?;
    client.repo_merge_options(&repo).await
}
