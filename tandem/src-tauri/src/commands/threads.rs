//! Threaded discussion on a local review comment: persist your reply,
//! then (when an agent is involved) launch a run that has the whole
//! thread as context and answers back into the same thread.

use tandem_agents::context::ThreadMessage;
use tandem_agents::conversation;
use tandem_agents::reply::{build_reply_prompt, ReplyContext};
use tandem_cache::{CommentRevisions, ReviewStore, SummaryStore};
use tandem_core::review::{
    CommentAuthorKind, CommentSeverity, DiffSide, LocalComment, NewLocalComment,
};
use tandem_core::TandemError;
use tauri::{AppHandle, Emitter, State};

use crate::commands::agents::launch_run;
use crate::commands::parse_repo;
use crate::commands::thread_context::{extract_file_diff, sibling_threads};
use crate::state::AppState;

/// Add your reply to a thread, then have `agent_name` answer it.
#[tauri::command]
pub async fn reply_to_comment(
    app: AppHandle,
    state: State<'_, AppState>,
    agent_name: String,
    comment_id: String,
    body: String,
) -> Result<String, TandemError> {
    if body.trim().is_empty() {
        return Err(TandemError::Config("reply cannot be empty".into()));
    }

    let root_id = {
        let cache = state.cache.lock().await;
        let root = resolve_root(&cache, &comment_id)?;
        add_reply(&cache, &root, &body)?;
        notify(&app, &root);
        root.id
    };

    launch_thread_run(app, &state, &agent_name, &root_id).await
}

/// Reply to the agent's PR summary, starting the discussion thread the
/// first time. The root is a PR-level comment seeded with the summary
/// text, so the agent answering sees what's being discussed and the
/// thread lives with the rest of the review instead of in its own silo.
#[tauri::command]
pub async fn reply_to_summary(
    app: AppHandle,
    state: State<'_, AppState>,
    agent_name: String,
    repo: String,
    number: u64,
    body: String,
) -> Result<String, TandemError> {
    if body.trim().is_empty() {
        return Err(TandemError::Config("reply cannot be empty".into()));
    }
    let repo = parse_repo(&repo)?;

    let root_id = {
        let cache = state.cache.lock().await;
        let root = summary_thread_root(&app, &cache, &repo, number)?;
        add_reply(&cache, &root, &body)?;
        notify(&app, &root);
        root.id
    };

    launch_thread_run(app, &state, &agent_name, &root_id).await
}

/// The summary's thread root, created on first reply and remembered on
/// the summary itself so a re-review keeps the same conversation.
fn summary_thread_root(
    app: &AppHandle,
    cache: &tandem_cache::Cache,
    repo: &tandem_core::github::RepoRef,
    number: u64,
) -> Result<LocalComment, TandemError> {
    let mut summary = cache
        .get_pr_summary(repo, number)?
        .ok_or_else(|| TandemError::Cache("there's no summary to reply to yet".into()))?;
    if let Some(existing) = summary
        .thread_root_id
        .as_deref()
        .map(|id| cache.get_comment(id))
        .transpose()?
        .flatten()
    {
        return Ok(existing);
    }
    let root = cache.add_comment(NewLocalComment {
        repo: repo.clone(),
        pr_number: number,
        head_sha: summary.head_sha.clone(),
        // PR-level: the summary is about the whole PR, not a line.
        path: String::new(),
        side: DiffSide::New,
        line: 0,
        end_line: None,
        body: summary.body.clone(),
        suggestion: None,
        author_kind: CommentAuthorKind::Agent,
        author_name: summary.agent_name.clone(),
        severity: CommentSeverity::Info,
        run_id: Some(summary.run_id.clone()),
        tag: None,
        parent_id: None,
        github_comment_id: None,
    })?;
    summary.thread_root_id = Some(root.id.clone());
    cache.put_pr_summary(&summary)?;
    if let Err(e) = app.emit("tandem://summary-updated", &summary) {
        tracing::warn!(error = %e, "failed to emit summary-updated");
    }
    Ok(root)
}

/// Append your message to a thread, inheriting the root's anchor.
fn add_reply(
    cache: &tandem_cache::Cache,
    root: &LocalComment,
    body: &str,
) -> Result<(), TandemError> {
    cache.add_comment(NewLocalComment {
        repo: root.repo.clone(),
        pr_number: root.pr_number,
        head_sha: root.head_sha.clone(),
        path: root.path.clone(),
        side: root.side,
        line: root.line,
        end_line: root.end_line,
        body: body.to_owned(),
        suggestion: None,
        author_kind: CommentAuthorKind::Human,
        author_name: "you".into(),
        severity: CommentSeverity::Info,
        run_id: None,
        tag: None,
        parent_id: Some(root.id.clone()),
        github_comment_id: None,
    })?;
    Ok(())
}

/// Summon an agent into an existing thread (the @mention path) without
/// adding another human message first.
#[tauri::command]
pub async fn mention_agent(
    app: AppHandle,
    state: State<'_, AppState>,
    agent_name: String,
    comment_id: String,
) -> Result<String, TandemError> {
    launch_thread_run(app, &state, &agent_name, &comment_id).await
}

fn resolve_root(
    cache: &tandem_cache::Cache,
    comment_id: &str,
) -> Result<LocalComment, TandemError> {
    let comment = cache
        .get_comment(comment_id)?
        .ok_or_else(|| TandemError::Cache(format!("comment not found: {comment_id}")))?;
    Ok(match comment.parent_id.as_deref() {
        Some(pid) => cache.get_comment(pid)?.unwrap_or(comment),
        None => comment,
    })
}

fn notify(app: &AppHandle, root: &LocalComment) {
    let payload = serde_json::json!({ "repo": root.repo.slug(), "number": root.pr_number });
    if let Err(e) = app.emit("tandem://comments-updated", payload) {
        tracing::warn!(error = %e, "failed to emit comments-updated");
    }
}

/// Launch an agent run that continues the thread rooted at
/// `comment_id`'s root and replies into it.
async fn launch_thread_run(
    app: AppHandle,
    state: &AppState,
    agent_name: &str,
    comment_id: &str,
) -> Result<String, TandemError> {
    let (root, spec, pr, diff_excerpt, thread, conversation) = {
        let cache = state.cache.lock().await;
        let root = resolve_root(&cache, comment_id)?;
        let spec = cache
            .list_agent_specs()?
            .into_iter()
            .find(|s| s.name == agent_name)
            .ok_or_else(|| TandemError::Agent(format!("no agent named {agent_name}")))?;
        let detail = cache
            .get_pr_detail(&root.repo, root.pr_number)?
            .ok_or_else(|| TandemError::Agent("PR not synced yet — open it first".into()))?;
        let raw = cache
            .get_raw_diff(&root.repo, root.pr_number, &detail.pull_request.head_sha)?
            .unwrap_or_default();

        let mut thread: Vec<ThreadMessage> = Vec::new();
        // When the thread discusses a GitHub comment, lead with it so
        // the agent sees what's being talked about.
        if let Some(gh_id) = root.github_comment_id {
            if let Some(gh) = detail.comments.iter().find(|c| c.id == gh_id) {
                thread.push(ThreadMessage {
                    author: format!("{} (on github)", gh.author.login),
                    body: gh.body.clone(),
                });
            }
        }
        let mut messages: Vec<_> = cache
            .list_comments(&root.repo, root.pr_number)?
            .into_iter()
            .filter(|c| c.id == root.id || c.parent_id.as_deref() == Some(root.id.as_str()))
            .collect();
        messages.sort_by_key(|c| c.created_at);
        for comment in messages {
            // Say when a message has been rewritten — especially the
            // agent's own, so it doesn't argue with wording the
            // reviewer deliberately changed.
            let edited = cache.revision_count(&comment.id).unwrap_or(0) > 0;
            thread.push(ThreadMessage {
                author: if edited {
                    format!("{} (edited by the reviewer)", comment.author_name)
                } else {
                    comment.author_name.clone()
                },
                body: comment.body.clone(),
            });
        }

        let excerpt = extract_file_diff(&raw, &root.path);
        let siblings = sibling_threads(&cache, &detail, &root)?;
        (
            root,
            spec,
            detail.pull_request,
            excerpt,
            thread,
            conversation::render(&siblings),
        )
    };

    let instructions = spec.prompt.clone();
    let pr_body = pr.body.clone();
    let pr_for_prompt = pr.clone();
    let repo = root.repo.clone();
    let side = match root.side {
        DiffSide::Old => "old".to_owned(),
        DiffSide::New => "new".to_owned(),
    };
    let target = Some(root.id.clone());
    launch_run(
        app,
        state,
        spec,
        repo,
        pr,
        "thread reply",
        target,
        None,
        move |run_id, comments_file, _workdir| {
            let ctx = ReplyContext {
                run_id: run_id.to_owned(),
                comments_file: comments_file.to_owned(),
                conversation,
                pr_body: pr_body.clone(),
                parent_id: root.id.clone(),
                path: root.path.clone(),
                side,
                line: root.line,
                diff_text: diff_excerpt,
            };
            build_reply_prompt(&ctx, &pr_for_prompt, &thread, &instructions)
        },
    )
    .await
}
