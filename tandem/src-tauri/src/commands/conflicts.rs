//! Merge-conflict analysis: compute the likely-conflicting files (PR
//! files ∩ base-branch changes since the merge-base), hand both sides
//! to an agent, and let it open a local general-comment discussion.

use tandem_agents::context::{
    build_conflict_prompt, build_resolve_prompt, ConflictContext, ConflictFile, ResolveContext,
};
use tandem_cache::ReviewStore;
use tandem_core::agent::RunStatus;
use tandem_core::TandemError;
use tauri::{AppHandle, State};

use crate::commands::agents::launch_run;
use crate::commands::parse_repo;
use crate::state::AppState;

const MAX_PATCH_CHARS: usize = 12_000;

#[tauri::command]
pub async fn start_conflict_resolution(
    app: AppHandle,
    state: State<'_, AppState>,
    agent_name: String,
    repo: String,
    number: u64,
) -> Result<String, TandemError> {
    let repo = parse_repo(&repo)?;
    let (spec, pr, raw_diff) = {
        let cache = state.cache.lock().await;
        // One conflict analysis per PR at a time.
        let already_running = cache.list_agent_runs(&repo, number)?.into_iter().any(|r| {
            r.purpose == "conflict analysis"
                && matches!(r.status, RunStatus::Starting | RunStatus::Running)
        });
        if already_running {
            return Err(TandemError::Config(
                "a conflict analysis is already running for this PR".into(),
            ));
        }
        let spec = cache
            .list_agent_specs()?
            .into_iter()
            .find(|s| s.name == agent_name)
            .ok_or_else(|| TandemError::Agent(format!("no agent named {agent_name}")))?;
        let detail = cache
            .get_pr_detail(&repo, number)?
            .ok_or_else(|| TandemError::Agent("PR not synced yet — open it first".into()))?;
        let raw = cache
            .get_raw_diff(&repo, number, &detail.pull_request.head_sha)?
            .unwrap_or_default();
        (spec, detail.pull_request, raw)
    };

    let client = state.github_client().await?;
    let pr_files = client.pr_files(&repo, number).await?;
    let base_changes = client
        .base_changes_since(&repo, &pr.head_sha, &pr.base_ref)
        .await?;

    let files: Vec<ConflictFile> = base_changes
        .into_iter()
        .filter(|(name, _)| pr_files.contains(name))
        .map(|(path, patch)| ConflictFile {
            path,
            base_patch: patch.map(|p| {
                if p.len() > MAX_PATCH_CHARS {
                    format!("{}\n… (patch truncated)", &p[..MAX_PATCH_CHARS])
                } else {
                    p
                }
            }),
        })
        .collect();
    if files.is_empty() {
        return Err(TandemError::Config(
            "couldn't identify overlapping files — the conflict may be in a binary or rename"
                .into(),
        ));
    }

    let pr_for_prompt = pr.clone();
    launch_run(
        app,
        &state,
        spec,
        repo,
        pr,
        "conflict analysis",
        None,
        None,
        move |run_id, comments_file, _workdir| {
            let ctx = ConflictContext {
                run_id: run_id.to_owned(),
                comments_file: comments_file.to_owned(),
                files,
                diff_text: raw_diff,
            };
            build_conflict_prompt(&ctx, &pr_for_prompt)
        },
    )
    .await
}

/// Resolve the conflicts rather than describe them: fetch base, start
/// the merge so the markers are real, and hand the tree to an agent.
/// Nothing is committed or pushed — that stays a human click.
#[tauri::command]
pub async fn start_conflict_fix(
    app: AppHandle,
    state: State<'_, AppState>,
    agent_name: String,
    repo: String,
    number: u64,
) -> Result<String, TandemError> {
    let repo = parse_repo(&repo)?;
    let (spec, pr, briefing) = {
        let cache = state.cache.lock().await;
        let busy = cache.list_agent_runs(&repo, number)?.into_iter().any(|r| {
            r.purpose.starts_with("conflict")
                && matches!(r.status, RunStatus::Starting | RunStatus::Running)
        });
        if busy {
            return Err(TandemError::Config(
                "a conflict run is already going for this PR".into(),
            ));
        }
        let spec = cache
            .list_agent_specs()?
            .into_iter()
            .find(|s| s.name == agent_name)
            .ok_or_else(|| TandemError::Agent(format!("no agent named {agent_name}")))?;
        let detail = cache
            .get_pr_detail(&repo, number)?
            .ok_or_else(|| TandemError::Agent("PR not synced yet — open it first".into()))?;
        // An earlier analysis saves the agent re-deriving the context.
        let briefing = cache
            .list_comments(&repo, number)?
            .into_iter()
            .filter(|c| c.tag.as_deref() == Some("conflict") && c.parent_id.is_none())
            .max_by_key(|c| c.created_at)
            .map(|c| c.body);
        (spec, detail.pull_request, briefing)
    };

    // Tandem fetches and merges: the agent's sandbox has no network, so
    // asking it to do this is how you get "git fetch: permission denied"
    // and a resolution written against commits it never saw.
    let dir = crate::commands::workspace::ensure_pr_checkout(&state, &repo, number).await?;
    let merge = crate::commands::merge_tree::prepare_merge(&dir, &pr.base_ref).await?;
    if merge.clean {
        return Err(TandemError::Config(
            "the base branch merges cleanly now — nothing to resolve".into(),
        ));
    }

    let instructions = spec.prompt.clone();
    let pr_for_prompt = pr.clone();
    let base_ref = pr.base_ref.clone();
    let conflicted = merge.conflicted;
    launch_run(
        app,
        &state,
        spec,
        repo,
        pr,
        "conflict resolution",
        None,
        Some(dir),
        move |run_id, comments_file, _workdir| {
            let ctx = ResolveContext {
                run_id: run_id.to_owned(),
                comments_file: comments_file.to_owned(),
                base_ref,
                conflicted,
                briefing,
            };
            build_resolve_prompt(&ctx, &pr_for_prompt, &instructions)
        },
    )
    .await
}

/// What the resolution run left behind, for the UI to offer a push.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ResolutionState {
    pub unresolved: Vec<String>,
    pub ready: bool,
}

#[tauri::command]
pub async fn conflict_resolution_state(
    state: State<'_, AppState>,
    repo: String,
    number: u64,
) -> Result<ResolutionState, TandemError> {
    let repo = parse_repo(&repo)?;
    let dir = state
        .dirs
        .worktrees_dir
        .join("agent-runs")
        .join(repo.slug())
        .join(number.to_string());
    if !dir.is_dir() {
        return Ok(ResolutionState {
            unresolved: Vec::new(),
            ready: false,
        });
    }
    let unresolved = crate::commands::merge_tree::unresolved(&dir).await?;
    // A merge in progress with nothing unmerged is a resolution waiting
    // to be pushed.
    let merging = dir.join(".git").join("MERGE_HEAD").exists();
    Ok(ResolutionState {
        ready: merging && unresolved.is_empty(),
        unresolved,
    })
}
