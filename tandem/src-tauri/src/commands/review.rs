//! Local review-comment commands. These comments live only in Tandem's
//! cache — nothing here talks to GitHub.

use tandem_cache::{CommentRevisions, ReviewStore, SummaryStore};
use tandem_core::review::{CommentRevision, CommentStatus, LocalComment, NewLocalComment};
use tandem_core::TandemError;
use tauri::{AppHandle, Emitter, State};

use crate::commands::parse_repo;
use crate::state::AppState;

fn notify_comments_changed(app: &AppHandle, repo: &str, number: u64) {
    let payload = serde_json::json!({ "repo": repo, "number": number });
    if let Err(e) = app.emit("tandem://comments-updated", payload) {
        tracing::warn!(error = %e, "failed to emit comments-updated");
    }
}

#[tauri::command]
pub async fn list_local_comments(
    state: State<'_, AppState>,
    repo: String,
    number: u64,
) -> Result<Vec<LocalComment>, TandemError> {
    let repo = parse_repo(&repo)?;
    state.cache.lock().await.list_comments(&repo, number)
}

#[tauri::command]
pub async fn add_local_comment(
    app: AppHandle,
    state: State<'_, AppState>,
    comment: NewLocalComment,
) -> Result<LocalComment, TandemError> {
    let slug = comment.repo.slug();
    let number = comment.pr_number;
    let created = state.cache.lock().await.add_comment(comment)?;
    notify_comments_changed(&app, &slug, number);
    Ok(created)
}

#[tauri::command]
pub async fn set_comment_status(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    status: CommentStatus,
    repo: String,
    number: u64,
) -> Result<(), TandemError> {
    state.cache.lock().await.set_comment_status(&id, status)?;
    notify_comments_changed(&app, &repo, number);
    Ok(())
}

#[tauri::command]
pub async fn update_comment_body(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    body: String,
    repo: String,
    number: u64,
) -> Result<(), TandemError> {
    {
        let cache = state.cache.lock().await;
        let current = cache
            .get_comment(&id)?
            .ok_or_else(|| TandemError::Cache(format!("comment not found: {id}")))?;
        // A no-op save shouldn't manufacture a version.
        if current.body != body {
            // Whose words are being replaced: the author's the first
            // time, yours on every edit after that, since only you can
            // edit a comment.
            let editor = if cache.revision_count(&id)? == 0 {
                current.author_name.clone()
            } else {
                "you".to_owned()
            };
            cache.record_revision(&id, &editor, &current.body)?;
            cache.update_comment_body(&id, &body)?;
        }
    }
    notify_comments_changed(&app, &repo, number);
    Ok(())
}

/// Previous versions of a comment, oldest first.
#[tauri::command]
pub async fn list_comment_revisions(
    state: State<'_, AppState>,
    id: String,
) -> Result<Vec<CommentRevision>, TandemError> {
    state.cache.lock().await.list_revisions(&id)
}

#[tauri::command]
pub async fn delete_local_comment(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    repo: String,
    number: u64,
) -> Result<(), TandemError> {
    state.cache.lock().await.delete_comment(&id)?;
    notify_comments_changed(&app, &repo, number);
    Ok(())
}

/// Drop the PR's agent-written summary. The next review writes a fresh
/// one rather than revising what was there.
#[tauri::command]
pub async fn delete_pr_summary(
    state: State<'_, AppState>,
    repo: String,
    number: u64,
) -> Result<(), TandemError> {
    let repo = crate::commands::parse_repo(&repo)?;
    state.cache.lock().await.delete_pr_summary(&repo, number)
}
