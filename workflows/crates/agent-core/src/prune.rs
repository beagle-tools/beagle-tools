//! Log retention: forget the *contents* of old runs (events + log file
//! reference) while keeping the run rows themselves as history.

use chrono::{DateTime, Utc};
use rusqlite::params;

use crate::state::Store;
use crate::store_rows::{collect_runs, run_from_row};
use crate::types::Run;
use crate::CoreError;

impl Store {
    /// Terminal runs that finished before `cutoff` and still carry artifacts
    /// (a log path). `log_path` is cleared by [`Store::prune_run_artifacts`],
    /// which is what makes this query idempotent across passes.
    pub fn prunable_runs(&self, cutoff: DateTime<Utc>) -> Result<Vec<Run>, CoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, project, agent, state, trigger, parent_run_id, params, tmux_session,
                    log_path, workdir, created_at, started_at, finished_at, exit_code, model, cost_usd, num_turns, usage_json
             FROM runs
             WHERE state NOT IN ('pending', 'running')
               AND log_path IS NOT NULL
               AND finished_at IS NOT NULL
               AND finished_at < ?1
             ORDER BY finished_at",
        )?;
        let rows = stmt.query_map(params![cutoff.to_rfc3339()], run_from_row)?;
        collect_runs(rows)
    }

    /// Delete a run outright — row and events. For no-op poller ticks that
    /// carry no history worth keeping. Files are the caller's job.
    pub fn delete_run(&self, run_id: &str) -> Result<(), CoreError> {
        self.conn
            .execute("DELETE FROM events WHERE run_id = ?1", params![run_id])?;
        self.conn
            .execute("DELETE FROM runs WHERE id = ?1", params![run_id])?;
        Ok(())
    }

    /// Drop a run's event feed and forget its log path. The run row (state,
    /// timings, exit code) stays for history. Deleting the log *file* is the
    /// caller's job — the store never touches the filesystem.
    pub fn prune_run_artifacts(&self, run_id: &str) -> Result<(), CoreError> {
        self.conn
            .execute("DELETE FROM events WHERE run_id = ?1", params![run_id])?;
        self.conn.execute(
            "UPDATE runs SET log_path = NULL WHERE id = ?1",
            params![run_id],
        )?;
        Ok(())
    }
}

#[cfg(test)]
// Tests should fail loudly on a broken fixture; unwrapping is the clearest way.
#[allow(clippy::unwrap_used)]
mod tests {
    use std::path::Path;

    use chrono::Duration;

    use crate::types::{RunState, Trigger};

    use super::*;

    fn temp_store(tag: &str) -> (Store, std::path::PathBuf) {
        let dir =
            std::env::temp_dir().join(format!("agent-core-prune-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = Store::open(&dir.join("state.db")).unwrap();
        (store, dir)
    }

    fn finished_run(store: &Store, agent: &str) -> Run {
        let run = store
            .create_pending_run("p", agent, Trigger::Manual, None, None, None)
            .unwrap();
        store
            .mark_running(
                &run.id,
                "agentd-x",
                Path::new("/tmp/x.jsonl"),
                Path::new("/tmp"),
            )
            .unwrap();
        store.insert_event(&run.id, "message", Some("hi")).unwrap();
        store
            .finish_run(&run.id, RunState::Succeeded, Some(0))
            .unwrap();
        run
    }

    #[test]
    fn prune_clears_events_and_log_path_but_keeps_the_run() {
        let (store, dir) = temp_store("clear");
        let run = finished_run(&store, "a");

        let future = Utc::now() + Duration::hours(1);
        let prunable = store.prunable_runs(future).unwrap();
        assert_eq!(prunable.len(), 1);
        assert_eq!(prunable[0].id, run.id);

        store.prune_run_artifacts(&run.id).unwrap();
        assert_eq!(
            store.events_after(&run.id, 0, 10).unwrap(),
            [] as [crate::types::Event; 0]
        );
        let kept = store.get_run(&run.id).unwrap().unwrap();
        assert_eq!(kept.state, RunState::Succeeded);
        assert!(kept.log_path.is_none());
        // Idempotent: log_path is gone, so the run no longer matches.
        assert_eq!(
            store.prunable_runs(future).unwrap(),
            [] as [crate::types::Run; 0]
        );

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn active_and_recent_runs_are_not_prunable() {
        let (store, dir) = temp_store("skip");
        // Pending: never prunable, no matter the cutoff.
        store
            .create_pending_run("p", "pending", Trigger::Cron, None, None, None)
            .unwrap();
        // Finished just now: not prunable against a cutoff in the past.
        finished_run(&store, "fresh");

        let past = Utc::now() - Duration::hours(4);
        assert_eq!(
            store.prunable_runs(past).unwrap(),
            [] as [crate::types::Run; 0]
        );

        let _ = std::fs::remove_dir_all(dir);
    }
}
