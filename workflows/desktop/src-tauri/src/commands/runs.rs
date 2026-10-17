//! Run listing, lookup and termination.

use agent_core::state::{RunFilter, Store};
use agent_core::types::{Run, RunState};
use tauri::State;

use crate::error::{lock_failed, AppError};
use crate::state::AppState;

/// Everything the runs table shows: active runs first (they are what you
/// are watching), then finished ones newest-first.
///
/// One query, not two, because "active" and "recent" are the same rows
/// ordered differently — splitting them in SQL would let a run change
/// state between the two queries and appear twice or not at all.
#[tauri::command]
pub fn list_runs(state: State<'_, AppState>, limit: Option<u32>) -> Result<Vec<Run>, AppError> {
    let store = state.store.lock().map_err(|_| lock_failed("store"))?;
    let mut runs = store
        .list_runs(&RunFilter {
            project: None,
            agent: None,
            states: None,
            limit: Some(limit.unwrap_or(200)),
        })
        .map_err(|e| AppError::new(format!("could not read runs: {e}")))?;
    runs.sort_by(|a, b| {
        let rank = |r: &Run| u8::from(!is_active(r.state));
        rank(a)
            .cmp(&rank(b))
            .then_with(|| b.created_at.cmp(&a.created_at))
    });
    Ok(runs)
}

const fn is_active(state: RunState) -> bool {
    matches!(state, RunState::Pending | RunState::Running)
}

#[tauri::command]
pub fn get_run(state: State<'_, AppState>, id: String) -> Result<Option<Run>, AppError> {
    let store = state.store.lock().map_err(|_| lock_failed("store"))?;
    store
        .get_run(&id)
        .map_err(|e| AppError::new(format!("could not read run {id}: {e}")))
}

/// The run's lineage, root first, so the UI can draw the chain it belongs
/// to. Bounded at 32 hops: a cycle here would mean a corrupt store, and an
/// unbounded walk would hang the window rather than report it.
#[tauri::command]
pub fn run_chain(state: State<'_, AppState>, id: String) -> Result<Vec<Run>, AppError> {
    let store = state.store.lock().map_err(|_| lock_failed("store"))?;
    let mut chain = Vec::new();
    let mut cursor = Some(id);
    for _ in 0..32 {
        let Some(current) = cursor.take() else { break };
        let Some(run) = store
            .get_run(&current)
            .map_err(|e| AppError::new(format!("could not read run {current}: {e}")))?
        else {
            break;
        };
        cursor.clone_from(&run.parent_run_id);
        chain.push(run);
    }
    chain.reverse();
    Ok(chain)
}

/// Kill a running run: tear down its tmux session, then mark it Killed.
///
/// Same order agentctl uses. The session may already be gone — that is not
/// an error, because the store update is what actually ends the run as far
/// as everything else is concerned.
#[tauri::command]
pub fn kill_run(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    let store = state.store.lock().map_err(|_| lock_failed("store"))?;
    let run = store
        .get_run(&id)
        .map_err(|e| AppError::new(format!("could not read run {id}: {e}")))?
        .ok_or_else(|| AppError::new(format!("run {id} not found")))?;
    if run.state != RunState::Running {
        return Err(AppError::new(format!(
            "run {id} is {}, and only a running run can be killed",
            run.state
        )));
    }
    if let Some(session) = &run.tmux_session {
        // Exact-name match on an augmented PATH: a Finder-launched app has
        // no /opt/homebrew/bin, and marking the run Killed while its session
        // keeps running is worse than refusing.
        agent_core::tmux::kill_session(session)
            .map_err(|e| AppError::new(format!("could not stop run {id}'s tmux session: {e}")))?;
    }
    finish_killed(&store, &id)
}

fn finish_killed(store: &Store, id: &str) -> Result<(), AppError> {
    store.finish_run(id, RunState::Killed, None).map_err(|e| {
        AppError::new(format!(
            "tmux session closed but the store did not update: {e}"
        ))
    })?;
    let _ = store.insert_event(id, "state", Some("killed"));
    Ok(())
}

/// Re-run one failed stage against the state its parent already produced.
///
/// This is the cheap half of a retry: a chain that died in
/// `linear-implement` has already paid for the research in
/// `linear-context`, and that stage wrote its output to a data dir the
/// new run reads through `$AGENTD_PARENT_DATA_DIR`. Pointing a fresh run
/// at the same parent resumes from there instead of re-deriving — on the
/// EI-490 chain that is $1.29 of context the retry does not repeat.
///
/// It is a NEW run, not a mutated one. Run rows are the record of what
/// actually happened, and rewriting a failure into a success would lose
/// that the first attempt failed at all.
#[tauri::command]
pub fn retry_run(state: State<'_, AppState>, id: String) -> Result<String, AppError> {
    let store = state.store.lock().map_err(|_| lock_failed("store"))?;
    let run = store
        .get_run(&id)
        .map_err(|e| AppError::new(format!("could not read run {id}: {e}")))?
        .ok_or_else(|| AppError::new(format!("run {id} not found")))?;

    if is_active(run.state) {
        return Err(AppError::new(format!(
            "{}/{} is still {} — kill it before retrying",
            run.project, run.agent, run.state
        )));
    }

    // The parent's data dir is the state being resumed from. Retention
    // deletes it along with the logs, so past that window a retry would
    // start a stage whose inputs are gone — it would run, and improvise.
    // Refuse instead, and say which run to restart from.
    if let Some(parent) = &run.parent_run_id {
        let dir = agent_core::paths::data_dir()
            .map_err(|e| AppError::new(format!("could not resolve the data dir: {e}")))?
            .join(parent);
        if !dir.is_dir() {
            return Err(AppError::new(format!(
                "the state this stage reads is gone — agentd pruned {parent}'s data dir with \
                 its logs. Re-run the chain from an earlier stage instead."
            )));
        }
    }

    let created = store
        .create_pending_run(
            &run.project,
            &run.agent,
            agent_core::types::Trigger::Manual,
            run.parent_run_id.as_deref(),
            run.params.as_deref(),
            run.model.as_deref(),
        )
        .map_err(|e| AppError::new(format!("could not queue the retry: {e}")))?;
    Ok(created.id)
}
