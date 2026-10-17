//! The ONLY commands that write to GitHub. Every one of them is behind
//! an explicit user click in the UI — agents have no path here, and the
//! agent prompt still forbids posting.

use tandem_cache::ReviewStore;
use tandem_core::review::{CommentStatus, DiffSide};
use tandem_core::TandemError;
use tandem_github::NewInlineComment;
use tauri::{AppHandle, Emitter, State};

use crate::commands::parse_repo;
use crate::state::AppState;

/// Post one local comment's body to GitHub. Routing:
/// - thread rooted on a GitHub review comment → reply in that thread
/// - anchored to a diff line → new inline review comment
/// - otherwise → plain PR comment
#[tauri::command]
pub async fn post_comment_to_github(
    app: AppHandle,
    state: State<'_, AppState>,
    comment_id: String,
) -> Result<u64, TandemError> {
    let (comment, thread_gh_id) = {
        let cache = state.cache.lock().await;
        let comment = cache
            .get_comment(&comment_id)?
            .ok_or_else(|| TandemError::Cache(format!("comment not found: {comment_id}")))?;
        if comment.posted_github_id.is_some() {
            return Err(TandemError::Config(
                "comment was already posted to GitHub".into(),
            ));
        }
        let root = match comment.parent_id.as_deref() {
            Some(pid) => cache.get_comment(pid)?,
            None => None,
        };
        // Reply target: the GitHub thread the root discusses, or the
        // review comment the root itself became when posted earlier.
        let thread_gh_id = comment
            .github_comment_id
            .or_else(|| root.as_ref().and_then(|r| r.github_comment_id))
            .or_else(|| root.as_ref().and_then(|r| r.posted_github_id));
        (comment, thread_gh_id)
    };

    let client = state.github_client().await?;
    let repo = comment.repo.clone();
    let number = comment.pr_number;

    // PR-level threads (no code anchor) always post as plain PR
    // comments — GitHub has no threaded replies for those.
    let github_id = if comment.path.is_empty() || comment.line == 0 {
        client
            .post_issue_comment(&repo, number, &comment.body)
            .await?
    } else if let Some(thread_id) = thread_gh_id {
        client
            .reply_to_review_comment(&repo, number, thread_id, &comment.body)
            .await?
    } else {
        client
            .create_review_comment(
                &repo,
                number,
                NewInlineComment {
                    commit_id: &comment.head_sha,
                    path: &comment.path,
                    line: comment.line,
                    right_side: comment.side == DiffSide::New,
                    body: &comment.body,
                },
            )
            .await?
    };

    {
        let cache = state.cache.lock().await;
        cache.set_comment_posted(&comment.id, github_id)?;
    }
    let payload = serde_json::json!({ "repo": repo.slug(), "number": number });
    if let Err(e) = app.emit("tandem://comments-updated", payload) {
        tracing::warn!(error = %e, "failed to emit comments-updated");
    }
    Ok(github_id)
}

/// Approve the PR on GitHub (explicit user action).
#[tauri::command]
pub async fn approve_pr(
    state: State<'_, AppState>,
    repo: String,
    number: u64,
    body: Option<String>,
) -> Result<(), TandemError> {
    let repo = parse_repo(&repo)?;
    let client = state.github_client().await?;
    client
        .approve_pull_request(
            &repo,
            number,
            body.as_deref().filter(|b| !b.trim().is_empty()),
        )
        .await
}

/// Post straight to GitHub with no local thread involved — still
/// strictly an explicit user action. Routing: reply into a review
/// thread, a new inline comment on a diff line, or a plain PR comment.
#[derive(Debug, serde::Deserialize)]
pub struct DirectReply {
    pub repo: String,
    pub number: u64,
    pub body: String,
    pub review_comment_id: Option<u64>,
    pub path: Option<String>,
    pub line: Option<u64>,
    pub side_new: Option<bool>,
}

#[tauri::command]
pub async fn reply_on_github(
    state: State<'_, AppState>,
    request: DirectReply,
) -> Result<u64, TandemError> {
    let DirectReply {
        repo,
        number,
        body,
        review_comment_id,
        path,
        line,
        side_new,
    } = request;
    if body.trim().is_empty() {
        return Err(TandemError::Config("reply cannot be empty".into()));
    }
    let repo = parse_repo(&repo)?;
    let client = state.github_client().await?;
    if let Some(id) = review_comment_id {
        return client
            .reply_to_review_comment(&repo, number, id, &body)
            .await;
    }
    if let (Some(path), Some(line)) = (path.as_deref(), line) {
        if !path.is_empty() && line > 0 {
            let head_sha = {
                let cache = state.cache.lock().await;
                cache
                    .get_pr_detail(&repo, number)?
                    .ok_or_else(|| TandemError::Agent("PR not synced yet — open it first".into()))?
                    .pull_request
                    .head_sha
            };
            return client
                .create_review_comment(
                    &repo,
                    number,
                    NewInlineComment {
                        commit_id: &head_sha,
                        path,
                        line,
                        right_side: side_new.unwrap_or(true),
                        body: &body,
                    },
                )
                .await;
        }
    }
    client.post_issue_comment(&repo, number, &body).await
}

/// Apply a comment's suggested change by committing it to the PR
/// branch via the Contents API — the "commit suggestion" button.
#[tauri::command]
pub async fn commit_suggestion(
    app: AppHandle,
    state: State<'_, AppState>,
    comment_id: String,
) -> Result<String, TandemError> {
    let comment = {
        let cache = state.cache.lock().await;
        cache
            .get_comment(&comment_id)?
            .ok_or_else(|| TandemError::Cache(format!("comment not found: {comment_id}")))?
    };
    let suggestion = comment
        .suggestion
        .clone()
        .ok_or_else(|| TandemError::Config("comment has no suggestion".into()))?;
    if comment.side != DiffSide::New || comment.path.is_empty() || comment.line == 0 {
        return Err(TandemError::Config(
            "suggestions can only be committed on new-side code lines".into(),
        ));
    }

    let detail = {
        let cache = state.cache.lock().await;
        cache
            .get_pr_detail(&comment.repo, comment.pr_number)?
            .ok_or_else(|| TandemError::Agent("PR not synced yet — open it first".into()))?
    };
    if detail.pull_request.head_sha != comment.head_sha {
        return Err(TandemError::Config(
            "the PR branch has new commits since this suggestion — refresh and re-review".into(),
        ));
    }

    let client = state.github_client().await?;
    let (content, blob_sha) = client
        .get_file(&comment.repo, &comment.path, &comment.head_sha)
        .await?;

    let had_trailing_newline = content.ends_with('\n');
    let mut lines: Vec<&str> = content.lines().collect();
    let start = usize::try_from(comment.line).unwrap_or(1) - 1;
    let end = usize::try_from(comment.end_line.unwrap_or(comment.line)).unwrap_or(1) - 1;
    if start > end || end >= lines.len() {
        return Err(TandemError::Config(format!(
            "suggestion lines {}..{} fall outside {} ({} lines)",
            comment.line,
            comment.end_line.unwrap_or(comment.line),
            comment.path,
            lines.len()
        )));
    }
    let replacement: Vec<&str> = suggestion.lines().collect();
    lines.splice(start..=end, replacement);
    let mut new_content = lines.join("\n");
    if had_trailing_newline {
        new_content.push('\n');
    }

    // Reads like any other commit on the branch: no agent name, no
    // tooling credit. Who suggested it is review history, not something
    // the repo's log needs to carry forever.
    let message = format!("Apply suggestion to {}:{}", comment.path, comment.line);
    let commit_sha = client
        .commit_file(
            &comment.repo,
            &comment.path,
            &detail.pull_request.head_ref,
            &message,
            &new_content,
            &blob_sha,
        )
        .await?;

    {
        let cache = state.cache.lock().await;
        cache.set_comment_committed(&comment.id, &commit_sha)?;
        cache.set_comment_status(&comment.id, CommentStatus::Resolved)?;
    }
    let payload = serde_json::json!({ "repo": comment.repo.slug(), "number": comment.pr_number });
    if let Err(e) = app.emit("tandem://comments-updated", payload) {
        tracing::warn!(error = %e, "failed to emit comments-updated");
    }
    Ok(commit_sha)
}

/// Resolve a GitHub review thread (explicit user action).
#[tauri::command]
pub async fn resolve_github_thread(
    state: State<'_, AppState>,
    thread_id: String,
) -> Result<(), TandemError> {
    let client = state.github_client().await?;
    client.resolve_review_thread(&thread_id).await
}

/// Take your own draft PR out of draft (explicit user action). The UI
/// only offers this on a draft you authored; GitHub rejects it
/// otherwise.
#[tauri::command]
pub async fn mark_ready_for_review(
    state: State<'_, AppState>,
    repo: String,
    number: u64,
) -> Result<(), TandemError> {
    let repo = parse_repo(&repo)?;
    let client = state.github_client().await?;
    // Fetch fresh to guarantee a node id (older cached PRs lack it).
    let pr = client.pull_request(&repo, number).await?;
    let node_id = pr.node_id.ok_or_else(|| TandemError::GithubApi {
        status: 0,
        message: "GitHub did not return a node id for this PR".into(),
    })?;
    client.mark_pr_ready_for_review(&node_id).await
}

/// Commit the resolved merge and push it to the PR branch. The agent
/// resolves; this is the click that makes it real, so it refuses
/// anything half-done rather than pushing markers to a branch.
#[tauri::command]
pub async fn push_conflict_resolution(
    app: AppHandle,
    state: State<'_, AppState>,
    repo: String,
    number: u64,
) -> Result<String, TandemError> {
    let repo = parse_repo(&repo)?;
    let dir = state
        .dirs
        .worktrees_dir
        .join("agent-runs")
        .join(repo.slug())
        .join(number.to_string());
    if !dir.join(".git").join("MERGE_HEAD").exists() {
        return Err(TandemError::Config(
            "no merge in progress for this PR — start a conflict resolution first".into(),
        ));
    }
    let left = crate::commands::merge_tree::unresolved(&dir).await?;
    if !left.is_empty() {
        return Err(TandemError::Config(format!(
            "still unresolved: {} — answer the agent's questions first",
            left.join(", ")
        )));
    }

    let head = {
        let cache = state.cache.lock().await;
        cache
            .get_pr_detail(&repo, number)?
            .ok_or_else(|| TandemError::Agent("PR not synced yet".into()))?
            .pull_request
            .head_ref
    };

    let refspec = format!("HEAD:{head}");
    for args in [
        ["add", "-A"].as_slice(),
        ["commit", "--no-edit"].as_slice(),
        ["push", "origin", &refspec].as_slice(),
    ] {
        let (ok, out, err) = crate::commands::merge_tree::git(args, &dir).await?;
        if !ok {
            return Err(TandemError::Config(format!(
                "`git {}` failed: {}",
                args.join(" "),
                if err.is_empty() { out } else { err }
            )));
        }
    }
    let (_, sha, _) = crate::commands::merge_tree::git(&["rev-parse", "HEAD"], &dir).await?;
    let payload = serde_json::json!({ "repo": repo.slug(), "number": number });
    if let Err(e) = app.emit("tandem://comments-updated", payload) {
        tracing::warn!(error = %e, "failed to emit comments-updated");
    }
    Ok(sha)
}
