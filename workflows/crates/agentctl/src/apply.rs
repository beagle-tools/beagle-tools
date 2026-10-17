//! `agentctl apply` — load manifests and reconcile the store's agent set.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::ExitCode;

use agent_core::state::Store;
use agent_core::{manifest, paths};
use anyhow::Result;

use crate::fmt::Table;
use crate::get::PROJECTS_META_KEY;

/// Load manifests from `dir` (default: [`paths::manifests_dir`]), report
/// issues as WARN lines on stderr, sync the store, and print a summary.
/// Exits non-zero when any manifest had issues.
pub fn apply(dir: Option<&Path>) -> Result<ExitCode> {
    let root = match dir {
        Some(d) => d.to_path_buf(),
        None => paths::manifests_dir()?,
    };
    let outcome = manifest::load_root(&root)?;
    for issue in &outcome.issues {
        eprintln!("WARN {}: {}", issue.path.display(), issue.message);
    }

    let mut store = Store::open_default()?;
    store.reconcile_agents(&outcome.agents)?;
    // The store's agents table has no project metadata; stash descriptions in
    // meta so `get projects` stays store-only.
    store.set_meta(
        PROJECTS_META_KEY,
        &serde_json::to_string(&outcome.projects)?,
    )?;

    let mut counts: BTreeMap<&str, usize> = outcome
        .projects
        .iter()
        .map(|p| (p.name.as_str(), 0))
        .collect();
    for agent in &outcome.agents {
        *counts.entry(agent.project.as_str()).or_default() += 1;
    }
    let mut table = Table::new(&["PROJECT", "AGENTS"]);
    for (name, count) in &counts {
        table.row(vec![(*name).to_owned(), count.to_string()]);
    }
    table.print();
    println!(
        "applied {} agents across {} projects",
        outcome.agents.len(),
        counts.len()
    );

    if outcome.issues.is_empty() {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::FAILURE)
    }
}
