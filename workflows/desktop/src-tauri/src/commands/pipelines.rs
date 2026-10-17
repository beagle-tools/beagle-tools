//! The jobs view: pipelines, not agents.
//!
//! `list_jobs` returns every agent, which is what the store holds and
//! not what a person picks from — nobody starts `linear-implement`. The
//! grouping lives in `agent_core::pipelines` so agenttop shows the same
//! set, and so "is this a job or a step" is answered once.

use agent_core::lineage::{Lineage, SubtreeCost};
use agent_core::state::{RunFilter, Store};
use agent_core::types::{AgentSpec, Run};
use tauri::State;

use crate::error::{lock_failed, AppError};
use crate::state::AppState;

/// How far back the jobs screens read. The run list elsewhere is capped
/// at 200 across every agent, which with pr-watch ticking every fifteen
/// minutes is only the last few chains — not "every run of this job".
const HISTORY_LIMIT: u32 = 600;

/// One node of a job's flow: an agent at a depth, depth-first from the
/// entry point (row 0). The union of every chain the job has run, so a
/// branching job shows every branch — see `agent_core::flow`.
#[derive(Debug, serde::Serialize)]
pub struct FlowNode {
    pub agent: String,
    pub depth: usize,
}

/// A job as the landing page lists it.
#[derive(Debug, serde::Serialize)]
pub struct PipelineSummary {
    pub project: String,
    pub name: String,
    pub runner: String,
    pub schedule: Option<String>,
    pub enabled: bool,
    /// The job's shape, entry point first. Just the entry until it runs.
    pub flow: Vec<FlowNode>,
    /// Steps observed downstream, ordered by depth.
    pub steps: Vec<String>,
    /// Which steps are in flight right now. Two chains of the same job
    /// can sit at different steps, so this is a list.
    pub active_steps: Vec<String>,
    /// How many runs are live across all its chains.
    pub active_runs: usize,
    /// The newest run of the entry point, for "last: succeeded".
    pub last: Option<Run>,
}

#[tauri::command]
pub fn list_pipelines(state: State<'_, AppState>) -> Result<Vec<PipelineSummary>, AppError> {
    let store = state.store.lock().map_err(|_| lock_failed("store"))?;
    let (specs, runs) = specs_and_runs(&store)?;
    let mut out: Vec<PipelineSummary> = agent_core::pipelines::pipelines(&specs, &runs)
        .into_iter()
        .map(|p| PipelineSummary {
            flow: flow(&p.entry.project, &p.entry.name, &runs),
            project: p.entry.project.clone(),
            name: p.entry.name.clone(),
            runner: p.entry.runner.to_string(),
            schedule: p.entry.schedule.clone(),
            enabled: p.entry.enabled,
            steps: p.steps.clone(),
            active_steps: p
                .active_steps()
                .into_iter()
                .map(ToOwned::to_owned)
                .collect(),
            active_runs: p.active.len(),
            last: p.last.cloned(),
        })
        .collect();
    // Grouped by project, alphabetical within: the page draws a section
    // per project, and positions that do not reshuffle when a run ends.
    out.sort_by(|a, b| a.project.cmp(&b.project).then_with(|| a.name.cmp(&b.name)));
    Ok(out)
}

/// One step of one run, at its depth in that run's chain.
#[derive(Debug, serde::Serialize)]
pub struct StepRow {
    pub run: Run,
    pub depth: usize,
}

/// One run of a job: its root, what the whole chain cost, and every
/// step it ran in chain order.
#[derive(Debug, serde::Serialize)]
pub struct ChainView {
    pub root: Run,
    pub cost: SubtreeCost,
    pub steps: Vec<StepRow>,
}

/// Everything one job's page needs, in one read: its shape and its runs
/// with their steps. Returned whole so the page never has to cross-
/// reference a second, separately capped list to find a step — which is
/// why clicking an older run's step used to show "Select a run.".
#[derive(Debug, serde::Serialize)]
pub struct PipelineDetail {
    pub project: String,
    pub name: String,
    pub flow: Vec<FlowNode>,
    pub active_steps: Vec<String>,
    /// Newest first.
    pub chains: Vec<ChainView>,
}

#[tauri::command]
pub fn pipeline_detail(
    state: State<'_, AppState>,
    project: String,
    name: String,
) -> Result<PipelineDetail, AppError> {
    let store = state.store.lock().map_err(|_| lock_failed("store"))?;
    let (specs, runs) = specs_and_runs(&store)?;
    let found = agent_core::pipelines::pipelines(&specs, &runs)
        .into_iter()
        .find(|p| p.entry.project == project && p.entry.name == name)
        .ok_or_else(|| AppError::new(format!("{project}/{name} is not a job")))?;

    let lineage = Lineage::build(&runs);
    let chains = lineage
        .roots()
        .iter()
        .filter(|root| root.project == project && root.agent == name)
        .map(|root| ChainView {
            cost: lineage.subtree_cost(root),
            steps: lineage
                .rows(Some(&root.id))
                .into_iter()
                .map(|(run, depth)| StepRow {
                    run: run.clone(),
                    depth,
                })
                .collect(),
            root: (*root).clone(),
        })
        .collect();

    Ok(PipelineDetail {
        flow: flow(&project, &name, &runs),
        active_steps: found
            .active_steps()
            .into_iter()
            .map(ToOwned::to_owned)
            .collect(),
        project,
        name,
        chains,
    })
}

fn flow(project: &str, name: &str, runs: &[Run]) -> Vec<FlowNode> {
    agent_core::pipelines::flow_tree(project, name, runs)
        .into_iter()
        .map(|(agent, depth)| FlowNode { agent, depth })
        .collect()
}

fn specs_and_runs(store: &Store) -> Result<(Vec<AgentSpec>, Vec<Run>), AppError> {
    let specs = store
        .list_agents()
        .map_err(|e| AppError::new(format!("could not read agents: {e}")))?;
    let runs = store
        .list_runs(&RunFilter {
            project: None,
            agent: None,
            states: None,
            limit: Some(HISTORY_LIMIT),
        })
        .map_err(|e| AppError::new(format!("could not read runs: {e}")))?;
    Ok((specs, runs))
}
