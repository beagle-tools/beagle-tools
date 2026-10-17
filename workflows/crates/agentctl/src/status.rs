//! `agentctl status` — one-screen summary of agents and runs.

use std::collections::BTreeMap;

use agent_core::state::{RunFilter, Store};
use agent_core::types::RunState;
use anyhow::Result;
use chrono::Utc;

use crate::fmt::{self, Table};

const ALL_STATES: [RunState; 6] = [
    RunState::Pending,
    RunState::Running,
    RunState::Succeeded,
    RunState::Failed,
    RunState::TimedOut,
    RunState::Killed,
];

/// Print agent counts by project, run counts by state, and the five most
/// recent runs.
pub fn status(store: &Store) -> Result<()> {
    let now = Utc::now();
    let agents = store.list_agents()?;
    let runs = store.list_runs(&RunFilter::default())?;

    println!("AGENTS ({} total)", agents.len());
    let mut by_project: BTreeMap<&str, usize> = BTreeMap::new();
    for agent in &agents {
        *by_project.entry(agent.project.as_str()).or_default() += 1;
    }
    if by_project.is_empty() {
        println!("No agents found. Run `agentctl apply` first.");
    } else {
        let mut table = Table::new(&["PROJECT", "AGENTS"]);
        for (project, count) in &by_project {
            table.row(vec![(*project).to_owned(), count.to_string()]);
        }
        table.print();
    }

    println!();
    println!("RUNS ({} total)", runs.len());
    let mut table = Table::new(&["STATE", "COUNT"]);
    for state in ALL_STATES {
        let count = runs.iter().filter(|run| run.state == state).count();
        table.row(vec![state.to_string(), count.to_string()]);
    }
    table.print();

    println!();
    println!("RECENT RUNS");
    if runs.is_empty() {
        println!("No runs recorded.");
        return Ok(());
    }
    let mut table = Table::new(&["RUN ID", "PROJECT", "AGENT", "STATE", "AGE"]);
    for run in runs.iter().take(5) {
        table.row(vec![
            run.id.clone(),
            run.project.clone(),
            run.agent.clone(),
            run.state.to_string(),
            fmt::age(run.created_at, now),
        ]);
    }
    table.print();
    Ok(())
}
