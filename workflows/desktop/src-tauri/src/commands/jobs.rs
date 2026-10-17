//! Job listing and manual triggering.
//!
//! A "job" is an agent addressed by name, with its declared parameters
//! enforced BEFORE anything is queued — the same contract `ag -j` has on
//! the command line. Validating here rather than in the UI means the rule
//! holds however the run was started.

use std::collections::BTreeMap;

use agent_core::types::{AgentSpec, Trigger};
use tauri::State;

use crate::error::{lock_failed, AppError};
use crate::state::AppState;

#[tauri::command]
pub fn list_jobs(state: State<'_, AppState>) -> Result<Vec<AgentSpec>, AppError> {
    let store = state.store.lock().map_err(|_| lock_failed("store"))?;
    let mut specs = store
        .list_agents()
        .map_err(|e| AppError::new(format!("could not read agents: {e}")))?;
    specs.sort_by(|a, b| a.project.cmp(&b.project).then_with(|| a.name.cmp(&b.name)));
    Ok(specs)
}

/// Queue a manual run. Returns the new run id.
#[tauri::command]
pub fn trigger_job(
    state: State<'_, AppState>,
    project: String,
    agent: String,
    params: BTreeMap<String, String>,
    model: Option<String>,
) -> Result<String, AppError> {
    let store = state.store.lock().map_err(|_| lock_failed("store"))?;
    let spec = store
        .get_agent(&project, &agent)
        .map_err(|e| AppError::new(format!("could not read agent {project}/{agent}: {e}")))?
        .ok_or_else(|| {
            AppError::new(format!(
                "agent {project}/{agent} is not in the store — run `agentctl apply` to sync manifests"
            ))
        })?;

    // Named one at a time so the message can list every missing parameter
    // at once. Being told about one, fixing it, and being told about the
    // next is a worse experience than being told about all three.
    let missing: Vec<&str> = spec
        .params
        .iter()
        .filter(|p| p.required && !params.contains_key(&p.name))
        .map(|p| p.name.as_str())
        .collect();
    if !missing.is_empty() {
        return Err(AppError::new(format!(
            "{project}/{agent} needs {} before it can run: {}",
            if missing.len() == 1 {
                "a parameter"
            } else {
                "parameters"
            },
            missing.join(", ")
        )));
    }

    let params_json = if params.is_empty() {
        None
    } else {
        Some(
            serde_json::to_string(&params)
                .map_err(|e| AppError::new(format!("could not encode parameters: {e}")))?,
        )
    };

    let created = store
        .create_pending_run(
            &project,
            &agent,
            Trigger::Manual,
            None,
            params_json.as_deref(),
            model.as_deref(),
        )
        .map_err(|e| AppError::new(format!("could not queue the run: {e}")))?;
    Ok(created.id)
}
