//! Turn pending runs into live tmux sessions — the "kubelet starts the pod"
//! step.

use std::path::PathBuf;
use std::process::Command;

use agent_core::state::Store;
use agent_core::types::{AgentSpec, Isolation, Run, RunState};
use agent_core::{ids, paths};
use anyhow::{bail, Context, Result};
use tracing::{info, warn};

use crate::runscript::build_run_script;
use crate::tmux;

/// Spawn every pending run whose agent spec still exists. Per-run failures
/// finalize that run as `Failed` (with an explanatory event) and never abort
/// the pass.
pub fn spawn_pending(store: &Store, notify_cfg: &crate::notify::NotifyConfig) -> Result<()> {
    for run in store.pending_runs()? {
        if let Err(e) = spawn_one(store, &run, notify_cfg) {
            warn!(run = %run.id, error = %format!("{e:#}"), "spawn pass error for run");
        }
    }
    Ok(())
}

fn spawn_one(store: &Store, run: &Run, notify_cfg: &crate::notify::NotifyConfig) -> Result<()> {
    let Some(spec) = store.get_agent(&run.project, &run.agent)? else {
        return fail_run(
            store,
            run,
            &format!(
                "unknown agent {}/{}: spec no longer in store",
                run.project, run.agent
            ),
            notify_cfg,
        );
    };

    // max_concurrent: leave the run Pending (queued) while the agent is at
    // its cap; a later pass drains the queue as runs finish.
    if let Some(cap) = spec.max_concurrent {
        if store.running_count(&run.project, &run.agent)? >= cap {
            return Ok(());
        }
    }

    let workdir = match resolve_workdir(&spec, &run.id) {
        Ok(dir) => dir,
        Err(e) => {
            return fail_run(
                store,
                run,
                &format!("worktree add failed: {e:#}"),
                notify_cfg,
            )
        }
    };

    let log_path = paths::logs_dir()?.join(format!("{}.jsonl", run.id));
    let root = chain_root(store, run);
    let script = match build_run_script(&spec, run, &log_path, &root) {
        Ok(script) => script,
        Err(e) => return fail_run(store, run, &format!("run setup failed: {e:#}"), notify_cfg),
    };
    let session = ids::tmux_session_name(&run.id);
    if let Err(e) = tmux::new_session(&session, &workdir, &script) {
        return fail_run(store, run, &format!("tmux spawn failed: {e:#}"), notify_cfg);
    }

    store.mark_running(&run.id, &session, &log_path, &workdir)?;
    store.insert_event(&run.id, "state", Some("running"))?;
    info!(run = %run.id, agent = %spec.qualified_name(), session = %session, "spawned run");
    Ok(())
}

/// The directory the run executes in: the declared workdir, or a fresh
/// detached git worktree of it when isolation is requested.
fn resolve_workdir(spec: &AgentSpec, run_id: &str) -> Result<PathBuf> {
    match spec.isolation {
        Isolation::None => Ok(spec.workdir.clone()),
        Isolation::Worktree => {
            let path = paths::worktrees_dir()?.join(run_id);
            let output = Command::new("git")
                .arg("-C")
                .arg(&spec.workdir)
                .args(["worktree", "add", "--detach"])
                .arg(&path)
                .output()
                .context("failed to execute git")?;
            if !output.status.success() {
                bail!("{}", String::from_utf8_lossy(&output.stderr).trim());
            }
            Ok(path)
        }
    }
}

/// The complete tmux script for a run: env exports (run identity + data
/// dirs for DAG data passing), then the runner body, then the exit marker.
/// The first run in this run's chain — itself when it has no parent.
///
/// Resolved HERE, by walking the store, because it is an exact
/// identifier and the only component that can compute it correctly is
/// the one holding the lineage. The alternative was asking the model to
/// derive it from a path in its prompt, which produced a run id that
/// did not exist and a write-up that retention then swept as an orphan.
///
/// Bounded at 32: a cycle would mean a corrupt store, and hanging a
/// spawn is a worse way to find that out than stopping.
fn chain_root(store: &Store, run: &Run) -> String {
    let mut current = run.id.clone();
    let mut parent = run.parent_run_id.clone();
    for _ in 0..32 {
        let Some(id) = parent else { break };
        match store.get_run(&id) {
            Ok(Some(next)) => {
                current = next.id;
                parent = next.parent_run_id;
            }
            _ => break,
        }
    }
    current
}

fn fail_run(
    store: &Store,
    run: &Run,
    reason: &str,
    notify_cfg: &crate::notify::NotifyConfig,
) -> Result<()> {
    store.insert_event(&run.id, "note", Some(reason))?;
    store.finish_run(&run.id, RunState::Failed, None)?;
    store.insert_event(&run.id, "state", Some("failed"))?;
    info!(run = %run.id, reason = %reason, "run finished: failed (never spawned)");
    crate::notify::run_finished(None, run, RunState::Failed, notify_cfg);
    Ok(())
}
