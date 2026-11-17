//! The jobs view: pipelines, not agents.
//!
//! `list_jobs` returns every agent, which is what the store holds and
//! not what a person picks from — nobody starts `ticket-implement`. The
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
    /// Its project is in `~/.config/agentd/ui.yaml`'s `hidden_projects`
    /// (demo, usually): left off the page unless "Show hidden" is on.
    pub hidden: bool,
    /// The newest run of the entry point, for "last: succeeded".
    pub last: Option<Run>,
}

#[tauri::command]
pub fn list_pipelines(state: State<'_, AppState>) -> Result<Vec<PipelineSummary>, AppError> {
    let store = state.store.lock().map_err(|_| lock_failed("store"))?;
    let (specs, runs) = specs_and_runs(&store)?;
    // Re-read each poll, so an edit to ui.yaml shows without a restart. A
    // file that does not parse hides nothing rather than failing the page.
    let view = agent_core::ui::UiConfig::load();
    let mut out: Vec<PipelineSummary> = agent_core::pipelines::pipelines(&specs, &runs)
        .into_iter()
        .map(|p| PipelineSummary {
            flow: flow(&p.entry.project, &p.entry.name, &runs, &specs),
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
            hidden: view.hides(&p.entry.project),
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
    /// The draft PR this run opened, if any — from any of its steps.
    pub pr: Option<String>,
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
        .map(|root| chain_view(&store, &lineage, root))
        .collect();

    Ok(PipelineDetail {
        flow: flow(&project, &name, &runs, &specs),
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

/// How many chains the all-runs page shows: the newest, which is what a
/// live page is for. The PR lookup per chain is not free.
const ALL_CHAINS_LIMIT: usize = 120;

/// Every job's runs, newest first — the all-runs page: the same chains a
/// job's page shows, for all of them at once. Leaves out the projects
/// `ui.yaml` hides unless `show_hidden`, and with `running_only` keeps the
/// chains with a step still pending or running.
#[tauri::command]
pub fn all_chains(
    state: State<'_, AppState>,
    show_hidden: bool,
    running_only: bool,
) -> Result<Vec<ChainView>, AppError> {
    let store = state.store.lock().map_err(|_| lock_failed("store"))?;
    let (_, runs) = specs_and_runs(&store)?;
    let view = agent_core::ui::UiConfig::load();
    let lineage = Lineage::build(&runs);
    Ok(lineage
        .roots()
        .iter()
        .filter(|root| show_hidden || !view.hides(&root.project))
        .filter(|root| {
            !running_only
                || lineage
                    .rows(Some(&root.id))
                    .iter()
                    .any(|(run, _)| !run.state.is_terminal())
        })
        .take(ALL_CHAINS_LIMIT)
        .map(|root| chain_view(&store, &lineage, root))
        .collect())
}

/// One run of a job as the runs list draws it.
fn chain_view(store: &Store, lineage: &Lineage<'_>, root: &Run) -> ChainView {
    let rows = lineage.rows(Some(&root.id));
    ChainView {
        pr: rows
            .iter()
            .find_map(|(run, _)| agent_core::links::run_pr(store, &run.id)),
        cost: lineage.subtree_cost(root),
        steps: rows
            .into_iter()
            .map(|(run, depth)| StepRow {
                run: run.clone(),
                depth,
            })
            .collect(),
        root: root.clone(),
    }
}

fn flow(project: &str, name: &str, runs: &[Run], specs: &[AgentSpec]) -> Vec<FlowNode> {
    agent_core::pipelines::flow_tree(project, name, runs, specs)
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
