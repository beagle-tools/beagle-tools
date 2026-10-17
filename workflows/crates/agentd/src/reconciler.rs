//! Desired state → observed state: mirror on-disk manifests into the store.

use std::path::Path;

use agent_core::manifest;
use agent_core::state::Store;
use agent_core::types::AgentSpec;
use anyhow::{Context, Result};
use tracing::warn;

/// Load manifests from `dir`, warn about per-file issues, and make the store's
/// agents table match what is on disk. Returns the loaded specs so the
/// scheduler can work from the same snapshot.
pub fn reconcile(store: &mut Store, dir: &Path) -> Result<Vec<AgentSpec>> {
    let outcome = manifest::load_root(dir)
        .with_context(|| format!("failed to scan manifests root {}", dir.display()))?;
    for issue in &outcome.issues {
        warn!(path = %issue.path.display(), message = %issue.message, "manifest issue");
    }
    store
        .reconcile_agents(&outcome.agents)
        .context("failed to reconcile agents into store")?;
    Ok(outcome.agents)
}
