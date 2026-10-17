//! Log retention: delete the log file and event feed of runs that finished
//! more than the configured window ago (rows stay as history), and — for
//! agents opting in via `noop_retention_minutes` — delete quiet poller
//! ticks outright.

use agent_core::state::{RunFilter, Store};
use agent_core::types::{AgentSpec, Run, RunState};
use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use tracing::{info, warn};

/// One retention pass. `retention_hours` of 0 disables log pruning entirely.
/// `now` is injected so tests control the clock.
/// Delete run write-ups older than this. Deliberately longer than the log
/// window: a summary exists to be read the next morning, by which time the
/// logs and the data dir are long gone.
pub(crate) const SUMMARY_RETENTION_HOURS: i64 = 24;

/// Meta key the daemon writes when it cannot reach GitHub, so agenttop can
/// say so rather than the problem living only in a log nobody opens.
pub const RETENTION_PROBLEM_KEY: &str = "retention_problem";

pub fn tick(
    store: &Store,
    specs: &[AgentSpec],
    now: DateTime<Utc>,
    retention_hours: u64,
) -> Result<()> {
    crate::summaries::prune_summaries(store, now);
    prune_noop_runs(store, specs, now)?;
    let Some(cutoff) = cutoff(now, retention_hours) else {
        return Ok(());
    };
    for run in store.prunable_runs(cutoff)? {
        if let Some(log) = &run.log_path {
            if let Err(e) = std::fs::remove_file(log) {
                if e.kind() != std::io::ErrorKind::NotFound {
                    warn!(run = %run.id, log = %log.display(), error = %e, "could not delete log");
                    continue; // keep log_path so a later pass retries
                }
            }
        }
        store.prune_run_artifacts(&run.id)?;
        // The run's data-passing scratch dir expires with its logs.
        if let Ok(data_root) = agent_core::paths::data_dir() {
            let _ = std::fs::remove_dir_all(data_root.join(&run.id));
        }
        info!(run = %run.id, "pruned logs + events (retention window passed)");
    }
    Ok(())
}

/// Delete no-op runs — succeeded, or timed out (e.g. the machine slept
/// mid-poll and the monitor caught up on wake), without writing an
/// announcement — once they're older than the agent's
/// `noop_retention_minutes`. Failed/killed runs are always kept, and the
/// newest run of all is kept so "last run" status stays visible.
fn prune_noop_runs(store: &Store, specs: &[AgentSpec], now: DateTime<Utc>) -> Result<()> {
    for spec in specs {
        let Some(minutes) = spec.noop_retention_minutes else {
            continue;
        };
        let Some(window) = i64::try_from(minutes).ok().and_then(Duration::try_minutes) else {
            continue;
        };
        let cutoff = now - window;
        let runs = store.list_runs(&RunFilter {
            project: Some(spec.project.clone()),
            agent: Some(spec.name.clone()),
            states: Some(vec![RunState::Succeeded, RunState::TimedOut]),
            limit: None,
        })?;
        // Newest first; index 0 is the keeper.
        for run in runs.iter().skip(1) {
            let expired = run.finished_at.is_some_and(|t| t < cutoff);
            if !expired || announced(run) {
                continue;
            }
            if let Some(log) = &run.log_path {
                let _ = std::fs::remove_file(log);
            }
            if let Ok(data_root) = agent_core::paths::data_dir() {
                let _ = std::fs::remove_dir_all(data_root.join(&run.id));
            }
            store.delete_run(&run.id)?;
            info!(run = %run.id, "deleted no-op run (noop_retention_minutes)");
        }
    }
    Ok(())
}

/// A run that wrote `$AGENTD_DATA_DIR/notify.txt` did real work — never
/// treated as a no-op.
fn announced(run: &Run) -> bool {
    agent_core::paths::data_dir().is_ok_and(|root| root.join(&run.id).join("notify.txt").is_file())
}

/// The instant before which finished runs are pruned; `None` = never prune.
fn cutoff(now: DateTime<Utc>, retention_hours: u64) -> Option<DateTime<Utc>> {
    if retention_hours == 0 {
        return None;
    }
    let window = i64::try_from(retention_hours)
        .ok()
        .and_then(Duration::try_hours)?;
    Some(now - window)
}

#[cfg(test)]
// Tests should fail loudly on a broken fixture; unwrapping is the clearest way.
#[allow(clippy::unwrap_used)]
mod tests {
    use std::path::PathBuf;

    use agent_core::types::{RunState, Trigger};

    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("agentd-retention-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn cutoff_disabled_at_zero_hours() {
        let now = Utc::now();
        assert!(cutoff(now, 0).is_none());
        assert_eq!(cutoff(now, 4).unwrap(), now - Duration::hours(4));
    }

    #[test]
    fn prunes_old_run_logs_but_spares_recent_ones() {
        let dir = temp_dir("prune");
        let store = Store::open(&dir.join("state.db")).unwrap();
        let log = dir.join("old.jsonl");
        std::fs::write(&log, "{}\n").unwrap();

        let run = store
            .create_pending_run("p", "a", Trigger::Manual, None, None, None)
            .unwrap();
        store.mark_running(&run.id, "agentd-x", &log, &dir).unwrap();
        store.insert_event(&run.id, "message", Some("hi")).unwrap();
        store
            .finish_run(&run.id, RunState::Succeeded, Some(0))
            .unwrap();

        // Within the window: nothing happens.
        tick(&store, &[], Utc::now(), 4).unwrap();
        assert!(log.is_file());
        assert_eq!(store.events_after(&run.id, 0, 10).unwrap().len(), 1);

        // Window elapsed (simulated by a future clock): log + events go.
        tick(&store, &[], Utc::now() + Duration::hours(5), 4).unwrap();
        assert!(!log.exists());
        assert_eq!(
            store.events_after(&run.id, 0, 10).unwrap(),
            [] as [agent_core::types::Event; 0]
        );
        assert!(store.get_run(&run.id).unwrap().is_some());

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn noop_runs_die_young_but_the_newest_survives() {
        let dir = temp_dir("noop");
        let store = Store::open(&dir.join("state.db")).unwrap();
        let spec = AgentSpec {
            project: "demo".to_owned(),
            name: "watch-poll".to_owned(),
            source: PathBuf::new(),
            runner: agent_core::types::Runner::Script,
            model: None,
            schedule: None,
            workdir: "/tmp".into(),
            prompt: None,
            command: Some("watch-poll".to_owned()),
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
            noop_retention_minutes: Some(1),
            timeout_minutes: 2,
            notify: None,
            notify_icon: None,
            notify_icon_failure: None,
            concurrency: agent_core::types::ConcurrencyPolicy::Forbid,
            isolation: agent_core::types::Isolation::None,
            enabled: true,
            entrypoint: None,
        };

        let mut ids = Vec::new();
        for _ in 0..3 {
            let run = store
                .create_pending_run("demo", "watch-poll", Trigger::Cron, None, None, None)
                .unwrap();
            store
                .mark_running(&run.id, "s", &dir.join("x.jsonl"), &dir)
                .unwrap();
            store
                .finish_run(&run.id, RunState::Succeeded, Some(0))
                .unwrap();
            ids.push(run.id);
        }
        // A timed-out poll (slept-through) is a no-op too; a failed run must
        // never be touched by the no-op pass.
        let slept = store
            .create_pending_run("demo", "watch-poll", Trigger::Cron, None, None, None)
            .unwrap();
        store
            .finish_run(&slept.id, RunState::TimedOut, None)
            .unwrap();
        let failed = store
            .create_pending_run("demo", "watch-poll", Trigger::Cron, None, None, None)
            .unwrap();
        store
            .finish_run(&failed.id, RunState::Failed, Some(1))
            .unwrap();

        // Not old enough yet: everything stays.
        tick(&store, std::slice::from_ref(&spec), Utc::now(), 4).unwrap();
        assert_eq!(store.list_runs(&RunFilter::default()).unwrap().len(), 5);

        // Two minutes later: old no-ops (succeeded AND timed-out) go; the
        // newest of them + the failed run stay.
        tick(
            &store,
            std::slice::from_ref(&spec),
            Utc::now() + Duration::minutes(2),
            4,
        )
        .unwrap();
        let left = store.list_runs(&RunFilter::default()).unwrap();
        assert_eq!(left.len(), 2);
        assert!(left.iter().any(|r| r.id == failed.id));
        // The newest prunable (the timed-out one, created last) is the
        // keeper; all three older succeeded no-ops are gone.
        assert!(left.iter().any(|r| r.id == slept.id));
        assert!(ids.iter().all(|id| !left.iter().any(|r| &r.id == id)));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn missing_log_file_still_prunes_events() {
        let dir = temp_dir("missing");
        let store = Store::open(&dir.join("state.db")).unwrap();
        let run = store
            .create_pending_run("p", "a", Trigger::Manual, None, None, None)
            .unwrap();
        store
            .mark_running(&run.id, "agentd-x", &dir.join("never-created.jsonl"), &dir)
            .unwrap();
        store
            .finish_run(&run.id, RunState::Failed, Some(1))
            .unwrap();

        tick(&store, &[], Utc::now() + Duration::hours(5), 4).unwrap();
        assert_eq!(
            store.events_after(&run.id, 0, 10).unwrap(),
            [] as [agent_core::types::Event; 0]
        );

        let _ = std::fs::remove_dir_all(dir);
    }
}
