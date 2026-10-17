//! `agentctl get` — list projects, agents, and runs.

use std::collections::BTreeMap;

use agent_core::state::{RunFilter, Store};
use agent_core::types::{ProjectSpec, RunState};
use anyhow::Result;
use chrono::Utc;

use crate::fmt::{self, Table};

/// Store meta key under which `apply` records project metadata (name and
/// description), since the store's agents table carries neither.
pub const PROJECTS_META_KEY: &str = "projects";

/// `get projects`: NAME, DESCRIPTION, AGENTS.
pub fn projects(store: &Store) -> Result<()> {
    let specs: Vec<ProjectSpec> = store
        .get_meta(PROJECTS_META_KEY)?
        .map(|json| serde_json::from_str(&json))
        .transpose()?
        .unwrap_or_default();
    let mut rows: BTreeMap<String, (String, usize)> = specs
        .into_iter()
        .map(|p| (p.name, (p.description.unwrap_or_default(), 0)))
        .collect();
    for agent in store.list_agents()? {
        rows.entry(agent.project).or_default().1 += 1;
    }
    if rows.is_empty() {
        println!("No projects found. Run `agentctl apply` first.");
        return Ok(());
    }
    let mut table = Table::new(&["NAME", "DESCRIPTION", "AGENTS"]);
    for (name, (description, count)) in rows {
        table.row(vec![name, description, count.to_string()]);
    }
    table.print();
    Ok(())
}

/// `get agents`: PROJECT, NAME, RUNNER, SCHEDULE, ENABLED, LAST RUN.
pub fn agents(store: &Store, project: Option<&str>) -> Result<()> {
    let now = Utc::now();
    let mut table = Table::new(&[
        "PROJECT", "NAME", "RUNNER", "SCHEDULE", "ENABLED", "LAST RUN",
    ]);
    for spec in store.list_agents()? {
        if project.is_some_and(|p| p != spec.project) {
            continue;
        }
        let last = store.latest_run(&spec.project, &spec.name)?.map_or_else(
            || "-".to_owned(),
            |run| format!("{} ({})", fmt::age(run.created_at, now), run.state),
        );
        table.row(vec![
            spec.project,
            spec.name,
            spec.runner.to_string(),
            spec.schedule.unwrap_or_else(|| "manual".to_owned()),
            spec.enabled.to_string(),
            last,
        ]);
    }
    if table.is_empty() {
        println!("No agents found. Run `agentctl apply` first.");
        return Ok(());
    }
    table.print();
    Ok(())
}

/// `get runs`: RUN ID, PROJECT, AGENT, STATE, TRIGGER, AGE, DURATION, EXIT.
pub fn runs(
    store: &Store,
    project: Option<String>,
    agent: Option<String>,
    state: Option<&str>,
    limit: u32,
) -> Result<()> {
    let states = state
        .map(str::parse::<RunState>)
        .transpose()?
        .map(|s| vec![s]);
    let filter = RunFilter {
        project,
        agent,
        states,
        limit: Some(limit),
    };
    let now = Utc::now();
    let mut table = Table::new(&[
        "RUN ID", "PROJECT", "AGENT", "STATE", "TRIGGER", "AGE", "DURATION", "COST", "EXIT",
    ]);
    for run in store.list_runs(&filter)? {
        let exit = run
            .exit_code
            .map_or_else(|| "-".to_owned(), |code| code.to_string());
        table.row(vec![
            run.id.clone(),
            run.project.clone(),
            run.agent.clone(),
            run.state.to_string(),
            run.trigger.to_string(),
            fmt::age(run.created_at, now),
            fmt::run_duration(&run, now),
            fmt::cost(run.cost_usd),
            exit,
        ]);
    }
    if table.is_empty() {
        println!("No runs found.");
        return Ok(());
    }
    table.print();
    Ok(())
}

/// `agentctl get jobs` — every runnable job and the parameters it declares,
/// so `-j` callers (and agents driving the CLI) can discover what to pass
/// without triggering a run to read the failure. `*` marks required.
pub fn jobs(store: &Store) -> Result<()> {
    let agents = store.list_agents()?;
    let mut table = Table::new(&["JOB", "RUNNER", "PARAMS", "ENABLED"]);
    for spec in &agents {
        table.row(vec![
            spec.qualified_name(),
            spec.runner.as_str().to_owned(),
            crate::job::describe_params(&spec.params),
            spec.enabled.to_string(),
        ]);
    }
    table.print();
    if agents.iter().any(|a| !a.params.is_empty()) {
        println!("\n* = required. Run one with: agentctl -j <job> -p key=value");
    }
    Ok(())
}
