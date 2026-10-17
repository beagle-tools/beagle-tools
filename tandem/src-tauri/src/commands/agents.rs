//! Agent commands: manage specs, start/cancel review runs, and pump run
//! events into the cache and the UI.

use chrono::Utc;
use tandem_agents::context::{build_prompt, ReviewContext};
use tandem_agents::conventions;
use tandem_agents::runner::{LocalProcessRunner, RunRequest};
use tandem_cache::{ReviewStore, SummaryStore};
use tandem_core::agent::{AgentRun, AgentSpec, RunEvent, RunStatus};
use tandem_core::github::RepoRef;
use tandem_core::TandemError;
use tauri::{AppHandle, Emitter, State};

use crate::commands::agent_pump::pump_events;
use crate::commands::parse_repo;
use crate::state::AppState;

#[tauri::command]
pub async fn list_agent_specs(state: State<'_, AppState>) -> Result<Vec<AgentSpec>, TandemError> {
    state.cache.lock().await.list_agent_specs()
}

#[tauri::command]
pub async fn save_agent_spec(
    state: State<'_, AppState>,
    spec: AgentSpec,
) -> Result<(), TandemError> {
    if spec.name.trim().is_empty() {
        return Err(TandemError::Config("agent name cannot be empty".into()));
    }
    state.cache.lock().await.put_agent_spec(&spec)
}

#[tauri::command]
pub async fn delete_agent_spec(
    state: State<'_, AppState>,
    name: String,
) -> Result<(), TandemError> {
    state.cache.lock().await.delete_agent_spec(&name)
}

#[tauri::command]
pub async fn list_agent_runs(
    state: State<'_, AppState>,
    repo: String,
    number: u64,
) -> Result<Vec<AgentRun>, TandemError> {
    let repo = parse_repo(&repo)?;
    state.cache.lock().await.list_agent_runs(&repo, number)
}

/// Every recent run across all repos/PRs — the agents dashboard.
#[tauri::command]
pub async fn list_all_agent_runs(state: State<'_, AppState>) -> Result<Vec<AgentRun>, TandemError> {
    state.cache.lock().await.list_recent_runs(200)
}

/// Replay a run's persisted JSONL log. The durable on-disk record
/// survives app restarts and reaches every session — unlike the
/// in-memory live stream (`tandem://agent-event`), which only reaches
/// whichever session happened to be listening when the events fired.
#[tauri::command]
pub async fn get_run_log(
    state: State<'_, AppState>,
    run_id: String,
) -> Result<Vec<RunEvent>, TandemError> {
    let run = state
        .cache
        .lock()
        .await
        .get_agent_run(&run_id)?
        .ok_or_else(|| TandemError::Config(format!("no run found for {run_id}")))?;
    let contents = tokio::fs::read_to_string(&run.log_path)
        .await
        .map_err(|e| TandemError::Config(format!("failed to read run log: {e}")))?;
    Ok(contents
        .lines()
        .filter_map(|line| serde_json::from_str::<RunEvent>(line).ok())
        .collect())
}

#[tauri::command]
pub async fn cancel_agent_run(
    app: AppHandle,
    state: State<'_, AppState>,
    run_id: String,
) -> Result<(), TandemError> {
    if let Some(mut handle) = state.runs.lock().await.remove(&run_id) {
        // Live run: the pump observes the kill and persists Cancelled.
        handle.cancel();
        return Ok(());
    }
    // No live handle — a zombie row from a previous app session. Mark
    // it cancelled directly so "stop" always visibly works.
    let run = {
        let cache = state.cache.lock().await;
        let Some(mut run) = cache.get_agent_run(&run_id)? else {
            return Ok(());
        };
        if !matches!(run.status, RunStatus::Starting | RunStatus::Running) {
            return Ok(());
        }
        run.status = RunStatus::Cancelled;
        run.finished_at = Some(Utc::now());
        cache.put_agent_run(&run)?;
        run
    };
    if let Err(e) = app.emit("tandem://run-updated", &run) {
        tracing::warn!(error = %e, "failed to emit run-updated");
    }
    Ok(())
}

/// Delete a run and its local conversation (the run's comments plus
/// replies under them). Cancels the process first if still executing
/// and removes the run's log directory on disk.
#[tauri::command]
pub async fn delete_agent_run(
    app: AppHandle,
    state: State<'_, AppState>,
    run_id: String,
) -> Result<(), TandemError> {
    if let Some(mut handle) = state.runs.lock().await.remove(&run_id) {
        handle.cancel();
    }
    let run = {
        let cache = state.cache.lock().await;
        let run = cache.get_agent_run(&run_id)?;
        cache.delete_agent_run(&run_id)?;
        run
    };
    let Some(run) = run else { return Ok(()) };
    if let Some(dir) = std::path::Path::new(&run.log_path).parent() {
        if let Err(e) = std::fs::remove_dir_all(dir) {
            tracing::warn!(error = %e, "failed to remove run dir");
        }
    }
    let payload = serde_json::json!({ "repo": run.repo_slug, "number": run.pr_number });
    if let Err(e) = app.emit("tandem://comments-updated", payload) {
        tracing::warn!(error = %e, "failed to emit comments-updated");
    }
    if let Err(e) = app.emit(
        "tandem://run-deleted",
        serde_json::json!({ "run_id": run_id }),
    ) {
        tracing::warn!(error = %e, "failed to emit run-deleted");
    }
    Ok(())
}

/// Start an agent review of a PR. The PR bundle must be synced first so
/// its diff is in the cache. Returns the run id; progress streams via
/// `tandem://agent-event` and comments land via `tandem://comments-updated`.
#[tauri::command]
pub async fn start_agent_review(
    app: AppHandle,
    state: State<'_, AppState>,
    agent_name: String,
    repo: String,
    number: u64,
) -> Result<String, TandemError> {
    let repo = parse_repo(&repo)?;
    let (spec, pr, raw_diff, existing_summary) = {
        let cache = state.cache.lock().await;
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
            .ok_or_else(|| TandemError::Agent("diff not cached yet — open the PR first".into()))?;
        // A re-review revises what's already there instead of starting over.
        let summary = cache.get_pr_summary(&repo, number)?.map(|s| s.body);
        (spec, detail.pull_request, raw, summary)
    };

    let instructions = spec.prompt.clone();
    let wants_summary = spec.summary;
    let pr_for_prompt = pr.clone();
    launch_run(
        app,
        &state,
        spec,
        repo,
        pr,
        "pr review",
        None,
        None,
        move |run_id, comments_file, workdir| {
            // The checkout is where the repo states its own rules; with
            // no checkout the agent reviews on general principles.
            let conventions = workdir
                .map(|dir| conventions::block(&conventions::collect(dir)))
                .unwrap_or_default();
            let ctx = ReviewContext {
                run_id: run_id.to_owned(),
                comments_file: comments_file.to_owned(),
                diff_text: raw_diff,
                wants_summary,
                existing_summary,
                conventions,
            };
            build_prompt(&ctx, &pr_for_prompt, &instructions)
        },
    )
    .await
}

/// Shared launch path for every agent run (fresh review or thread
/// reply): create the run dir, spawn the process, persist the run row,
/// stash the cancel handle, and start the event pump.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn launch_run(
    app: AppHandle,
    state: &AppState,
    spec: AgentSpec,
    repo: RepoRef,
    pr: tandem_core::github::PullRequest,
    purpose: &str,
    target_comment_id: Option<String>,
    // A checkout the caller prepared and this must not disturb: a
    // conflict resolution leaves a merge in progress, and re-running
    // `gh pr checkout` over it fails — which dropped the run back to no
    // workdir at all, so the agent could read the conflicted file but
    // not write it.
    prepared: Option<std::path::PathBuf>,
    build_prompt_fn: impl FnOnce(&str, &str, Option<&std::path::Path>) -> String,
) -> Result<String, TandemError> {
    let run_id = uuid::Uuid::new_v4().to_string();
    let run_dir = state.dirs.runs_dir.join(&run_id);
    let comments_file = run_dir
        .join("comments.jsonl")
        .to_string_lossy()
        .into_owned();
    // Best-effort: give the run a real checkout to work from (git log,
    // file tree) instead of just the diff text in its prompt. Never
    // blocks the run — no `gh`, no network, private-repo auth trouble,
    // etc. all just fall back to today's diff-only behavior. Done
    // before the prompt because the prompt reads the repo's own
    // convention files out of it.
    let workdir = match prepared {
        Some(path) => Some(path),
        None => match crate::commands::workspace::ensure_pr_checkout(state, &repo, pr.number).await
        {
            Ok(path) => Some(path),
            Err(e) => {
                tracing::warn!(error = %e, repo = %repo.slug(), number = pr.number, "no local checkout for this run — continuing with diff-only context");
                None
            }
        },
    };

    let prompt = build_prompt_fn(&run_id, &comments_file, workdir.as_deref());

    let request = RunRequest {
        spec: spec.clone(),
        prompt,
        run_id: run_id.clone(),
        run_dir,
        workdir,
    };
    let handle = LocalProcessRunner.start(request)?;
    let log_path = handle.log_path.to_string_lossy().into_owned();
    let (events, cancel) = handle.split();

    let run = AgentRun {
        run_id: run_id.clone(),
        agent_name: spec.name.clone(),
        repo_slug: repo.slug(),
        pr_number: pr.number,
        head_sha: pr.head_sha.clone(),
        status: RunStatus::Starting,
        started_at: Utc::now(),
        finished_at: None,
        log_path,
        comment_count: 0,
        purpose: purpose.to_owned(),
        target_comment_id,
        error: None,
    };
    {
        let cache = state.cache.lock().await;
        cache.put_agent_run(&run)?;
    }
    state.runs.lock().await.insert(run_id.clone(), cancel);

    let head_sha = pr.head_sha.clone();
    tokio::spawn(pump_events(app, events, run, spec, repo, head_sha));
    Ok(run_id)
}
