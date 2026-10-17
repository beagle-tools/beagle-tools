//! The DAG page's data: declared topology, observed lineage, manifests.
//!
//! Both halves of agenttop's `d` page, plus what the graph view needs.
//! The lineage walk and the subtree-cost rule come from
//! `agent_core::lineage`, shared with agenttop — the two views draw the
//! same DAG because they compute it with the same code, not because
//! someone kept two implementations in step.

use agent_core::lineage::{Lineage, SubtreeCost};
use agent_core::state::{RunFilter, Store};
use agent_core::types::{AgentSpec, Run};
use tauri::State;

use crate::error::{lock_failed, AppError};
use crate::state::AppState;

/// One row of the lineage forest, flattened for rendering.
#[derive(Debug, serde::Serialize)]
pub struct ForestRow {
    pub run: Run,
    pub depth: usize,
    /// Only on a root: what the whole chain cost. The question this page
    /// answers is "what did that ticket cost", which spans the chain.
    pub chain_cost: Option<SubtreeCost>,
    /// Ids of the runs this one triggered, so the graph can draw edges
    /// without re-deriving the parent map it was just given.
    pub children: Vec<String>,
}

#[tauri::command]
pub fn run_forest(
    state: State<'_, AppState>,
    focus: Option<String>,
) -> Result<Vec<ForestRow>, AppError> {
    let store = state.store.lock().map_err(|_| lock_failed("store"))?;
    let runs = all_runs(&store)?;
    let lineage = Lineage::build(&runs);
    Ok(lineage
        .rows(focus.as_deref())
        .into_iter()
        .map(|(run, depth)| ForestRow {
            chain_cost: (depth == 0).then(|| lineage.subtree_cost(run)),
            children: lineage
                .children_of(&run.id)
                .iter()
                .map(|c| c.id.clone())
                .collect(),
            depth,
            run: run.clone(),
        })
        .collect())
}

/// An agent as the topology pane shows it: what it is, and how its last
/// run went.
#[derive(Debug, serde::Serialize)]
pub struct AgentNode {
    pub spec: AgentSpec,
    pub latest: Option<Run>,
}

/// Declared structure, grouped by project — the manifests, not the runs.
/// This is the half of the page that is true even when nothing has ever
/// run.
#[tauri::command]
pub fn agent_graph(state: State<'_, AppState>) -> Result<Vec<AgentNode>, AppError> {
    let store = state.store.lock().map_err(|_| lock_failed("store"))?;
    let runs = all_runs(&store)?;
    let mut specs = store
        .list_agents()
        .map_err(|e| AppError::new(format!("could not read agents: {e}")))?;
    specs.sort_by(|a, b| a.project.cmp(&b.project).then_with(|| a.name.cmp(&b.name)));

    Ok(specs
        .into_iter()
        .map(|spec| {
            // Newest run for this agent. `runs` is already newest-first
            // from the store, so the first match wins.
            let latest = runs
                .iter()
                .find(|r| r.project == spec.project && r.agent == spec.name)
                .cloned();
            AgentNode { spec, latest }
        })
        .collect())
}

/// The selected agent's manifest, verbatim — agenttop's CONFIG pane.
///
/// Reads the path the store recorded when the manifest was applied, so
/// what shows here is the file that produced the running config, not
/// whatever is on disk under a similar name.
#[tauri::command]
pub fn read_manifest(
    state: State<'_, AppState>,
    project: String,
    agent: String,
) -> Result<String, AppError> {
    let source = {
        let store = state.store.lock().map_err(|_| lock_failed("store"))?;
        store
            .get_agent(&project, &agent)
            .map_err(|e| AppError::new(format!("could not read agent {project}/{agent}: {e}")))?
            .ok_or_else(|| AppError::new(format!("agent {project}/{agent} not found")))?
            .source
    };
    if source.as_os_str().is_empty() {
        return Err(AppError::new(format!(
            "{project}/{agent} has no manifest path recorded — re-run `agentctl apply`"
        )));
    }
    std::fs::read_to_string(&source)
        .map_err(|e| AppError::new(format!("could not read {}: {e}", source.display())))
}

fn all_runs(store: &Store) -> Result<Vec<Run>, AppError> {
    store
        .list_runs(&RunFilter {
            project: None,
            agent: None,
            states: None,
            limit: Some(200),
        })
        .map_err(|e| AppError::new(format!("could not read runs: {e}")))
}
