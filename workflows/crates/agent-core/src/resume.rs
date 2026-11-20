//! Resuming a chain at the step that failed, instead of from the top.
//!
//! A step reads what the steps before it handed over: its parent's data
//! dir (`$AGENTD_PARENT_DATA_DIR`) and, for some jobs, the chain root's
//! (`$AGENTD_ROOT_RUN_ID`). agentd keeps those dirs for
//! [`DATA_RETENTION_HOURS`], so for that long a failed, timed-out or killed
//! step can be run again **in place**: a new run of the same step, with the
//! same parent, params and model. It joins the same chain, reads the same
//! handover, and triggers the rest of the chain as the original would have.
//!
//! agentctl (`agentctl resume <run>`), agenttop and Agentdesk all go
//! through [`resume_run`], so the three agree on what can be resumed.

use std::path::Path;

use crate::state::Store;
use crate::types::{Run, RunState};
use crate::CoreError;

/// How long agentd keeps a finished run's data dir — the window in which a
/// step after it can be resumed. A week; the dirs are a few KB each.
pub const DATA_RETENTION_HOURS: i64 = 24 * 7;

/// How a resumed run's first event begins: a `note` (so it is never
/// counted as an ingested log line), then the original run id.
pub const RESUMED_FROM: &str = "resumed from ";

/// Whether `run` is in a state that can be resumed. Pending, running and
/// succeeded steps cannot: there is nothing to pick up.
pub fn resumable(run: &Run) -> bool {
    matches!(
        run.state,
        RunState::Failed | RunState::TimedOut | RunState::Killed
    )
}

/// Queue a new run of `run_id`'s step, in its chain. `data_root` is the
/// daemon's data dir (`paths::data_dir()`), passed in so tests use their
/// own.
///
/// Refuses, with the reason, when the step did not fail, when its job no
/// longer exists, or when the handover it needs has been pruned — in that
/// case the chain has to start over from its entry point.
pub fn resume_run(store: &Store, run_id: &str, data_root: &Path) -> Result<Run, CoreError> {
    let run = store
        .get_run(run_id)?
        .ok_or_else(|| CoreError::Other(format!("run {run_id} not found")))?;
    if !resumable(&run) {
        return Err(CoreError::Other(format!(
            "run {run_id} is {}; only a failed, timed-out or killed step can be resumed",
            run.state.as_str()
        )));
    }
    if store.get_agent(&run.project, &run.agent)?.is_none() {
        return Err(CoreError::Other(format!(
            "job {}/{} no longer exists",
            run.project, run.agent
        )));
    }
    for ancestor in handover_sources(store, &run)? {
        if !data_root.join(&ancestor).is_dir() {
            return Err(CoreError::Other(format!(
                "the handover from {ancestor} is gone (kept {DATA_RETENTION_HOURS}h) — \
                 start the job again from its entry point"
            )));
        }
    }
    let created = store.create_pending_run(
        &run.project,
        &run.agent,
        run.trigger,
        run.parent_run_id.as_deref(),
        run.params.as_deref(),
        run.model.as_deref(),
    )?;
    store.insert_event(
        &created.id,
        "note",
        Some(&format!("{RESUMED_FROM}{}", run.id)),
    )?;
    Ok(created)
}

/// The runs whose data dirs a resumed step can read: its parent and the
/// chain root. Empty for an entry point, which reads nothing handed over.
fn handover_sources(store: &Store, run: &Run) -> Result<Vec<String>, CoreError> {
    let Some(parent) = run.parent_run_id.clone() else {
        return Ok(Vec::new());
    };
    let mut sources = vec![parent.clone()];
    let mut cursor = parent;
    // Bounded: lineage is a tree, but a corrupt cycle must not hang a UI.
    for _ in 0..64 {
        let found = store.get_run(&cursor)?.ok_or_else(|| {
            CoreError::Other(format!(
                "run {cursor} earlier in the chain is gone — start the job again from its entry point"
            ))
        })?;
        match found.parent_run_id {
            Some(up) => cursor = up,
            None => break,
        }
    }
    if !sources.contains(&cursor) {
        sources.push(cursor);
    }
    Ok(sources)
}

#[cfg(test)]
// Tests should fail loudly on a broken fixture; unwrapping is the clearest way.
#[allow(clippy::unwrap_used)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::types::Trigger;

    fn setup(tag: &str) -> (PathBuf, Store) {
        let dir = std::env::temp_dir().join(format!("agentd-resume-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("data")).unwrap();
        let mut store = Store::open(&dir.join("state.db")).unwrap();
        store
            .reconcile_agents(&[spec("gate"), spec("step")])
            .unwrap();
        (dir, store)
    }

    fn spec(name: &str) -> crate::types::AgentSpec {
        crate::types::AgentSpec {
            project: "p".to_owned(),
            name: name.to_owned(),
            source: PathBuf::new(),
            purpose: None,
            runner: crate::types::Runner::Script,
            model: None,
            schedule: None,
            timezone: crate::types::ScheduleTz::Utc,
            workdir: "/tmp".into(),
            prompt: None,
            command: Some("true".to_owned()),
            allowed_tools: None,
            disallowed_tools: None,
            append_system_prompt: None,
            add_dirs: Vec::new(),
            params: Vec::new(),
            requires_mcp: Vec::new(),
            mcp_config: None,
            env: std::collections::BTreeMap::default(),
            env_file: None,
            database: false,
            max_concurrent: None,
            noop_retention_minutes: None,
            notify: None,
            notify_icon: None,
            notify_icon_failure: None,
            timeout_minutes: 30,
            concurrency: crate::types::ConcurrencyPolicy::Forbid,
            isolation: crate::types::Isolation::None,
            sandbox: crate::types::CodexSandbox::Off,
            enabled: true,
            entrypoint: None,
            triggers: Vec::new(),
        }
    }

    /// gate (succeeded) → step (in `state`), with params and a model.
    fn chain(store: &Store, data: &Path, state: RunState) -> (Run, Run) {
        let gate = store
            .create_pending_run("p", "gate", Trigger::Cron, None, None, None)
            .unwrap();
        store
            .finish_run(&gate.id, RunState::Succeeded, Some(0))
            .unwrap();
        std::fs::create_dir_all(data.join(&gate.id)).unwrap();
        let step = store
            .create_pending_run(
                "p",
                "step",
                Trigger::Manual,
                Some(&gate.id),
                Some(r#"{"rca":"x"}"#),
                Some("opus"),
            )
            .unwrap();
        store.finish_run(&step.id, state, Some(1)).unwrap();
        (gate, store.get_run(&step.id).unwrap().unwrap())
    }

    #[test]
    fn a_failed_step_resumes_in_its_chain_with_its_params() {
        let (dir, store) = setup("ok");
        let data = dir.join("data");
        let (gate, step) = chain(&store, &data, RunState::TimedOut);

        let again = resume_run(&store, &step.id, &data).unwrap();

        assert_eq!(again.agent, "step");
        assert_eq!(again.state, RunState::Pending);
        assert_eq!(again.parent_run_id.as_deref(), Some(gate.id.as_str()));
        assert_eq!(again.params.as_deref(), Some(r#"{"rca":"x"}"#));
        assert_eq!(again.model.as_deref(), Some("opus"));
        let events = store.events_after(&again.id, 0, 10).unwrap();
        assert_eq!(events[0].kind, "note");
        assert_eq!(
            events[0].payload.as_deref(),
            Some(format!("{RESUMED_FROM}{}", step.id).as_str())
        );
        assert_eq!(store.log_event_count(&again.id).unwrap(), 0);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_succeeded_step_is_not_resumable() {
        let (dir, store) = setup("succeeded");
        let data = dir.join("data");
        let (_, step) = chain(&store, &data, RunState::Succeeded);
        let err = resume_run(&store, &step.id, &data).unwrap_err();
        assert!(err.to_string().contains("only a failed"), "{err}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_pruned_handover_refuses_rather_than_running_blind() {
        let (dir, store) = setup("pruned");
        let data = dir.join("data");
        let (gate, step) = chain(&store, &data, RunState::Failed);
        std::fs::remove_dir_all(data.join(&gate.id)).unwrap();
        let err = resume_run(&store, &step.id, &data).unwrap_err();
        assert!(err.to_string().contains("start the job again"), "{err}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn an_entry_point_resumes_without_any_handover() {
        let (dir, store) = setup("entry");
        let gate = store
            .create_pending_run(
                "p",
                "gate",
                Trigger::Cron,
                None,
                Some(r#"{"w":"12"}"#),
                None,
            )
            .unwrap();
        store
            .finish_run(&gate.id, RunState::TimedOut, None)
            .unwrap();
        let again = resume_run(&store, &gate.id, &dir.join("data")).unwrap();
        assert_eq!(again.parent_run_id, None);
        assert_eq!(again.params.as_deref(), Some(r#"{"w":"12"}"#));
        let _ = std::fs::remove_dir_all(dir);
    }
}
