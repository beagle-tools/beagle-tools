//! Log retention: forget the *contents* of old runs (events + log file
//! reference) while keeping the run rows themselves as history.

use chrono::{DateTime, Utc};
use rusqlite::params;

use crate::state::Store;
use crate::store_rows::{collect_runs, run_from_row};
use crate::types::Run;
use crate::CoreError;

/// The events [`Store::prune_run_artifacts`] keeps, as an SQL predicate.
pub const KEPT_EVENTS: &str =
    "kind IN ('state', 'note', 'result') OR (kind = 'raw' AND payload LIKE 'ERROR %')";

impl Store {
    /// Delete what [`Store::prune_run_artifacts`] kept, for runs that
    /// finished before `cutoff` — the end of the data-retention window.
    pub fn prune_kept_events(&self, cutoff: DateTime<Utc>) -> Result<usize, CoreError> {
        Ok(self.conn.execute(
            "DELETE FROM events WHERE run_id IN (
                 SELECT id FROM runs WHERE finished_at IS NOT NULL AND finished_at < ?1)",
            params![cutoff.to_rfc3339()],
        )?)
    }

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
    ///
    /// The evidence of what went wrong survives: state changes, notes, the
    /// runner's final `result`, and every `ERROR` line. They are a few rows
    /// per run, and without them a failure older than the log window has no
    /// reason attached — `agentctl errors` reads them. They go with the data
    /// dir, after a week ([`Store::prune_kept_events`]).
    pub fn prune_run_artifacts(&self, run_id: &str) -> Result<(), CoreError> {
        self.conn.execute(
            &format!("DELETE FROM events WHERE run_id = ?1 AND NOT ({KEPT_EVENTS})"),
            params![run_id],
        )?;
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
        // Only the evidence kinds are left — here, the state changes.
        assert!(store
            .events_after(&run.id, 0, 10)
            .unwrap()
            .iter()
            .all(|e| e.kind == "state"));
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
    fn the_evidence_of_a_failure_outlives_the_log_then_goes_with_the_data_dir() {
        let (store, dir) = temp_store("evidence");
        let run = finished_run(&store, "a");
        store
            .insert_event(&run.id, "raw", Some("INFO a: fine"))
            .unwrap();
        store
            .insert_event(&run.id, "raw", Some("ERROR a: broke"))
            .unwrap();
        store.insert_event(&run.id, "result", Some("{}")).unwrap();

        store.prune_run_artifacts(&run.id).unwrap();
        let kinds: Vec<(String, Option<String>)> = store
            .events_after(&run.id, 0, 50)
            .unwrap()
            .into_iter()
            .map(|e| (e.kind, e.payload))
            .collect();
        assert!(kinds
            .iter()
            .any(|(k, p)| k == "raw" && p.as_deref() == Some("ERROR a: broke")));
        assert!(kinds.iter().any(|(k, _)| k == "result"));
        assert!(!kinds
            .iter()
            .any(|(k, p)| k == "message" || p.as_deref() == Some("INFO a: fine")));

        // Not yet past the window: kept.
        assert_eq!(
            store
                .prune_kept_events(Utc::now() - Duration::hours(1))
                .unwrap(),
            0
        );
        assert!(
            store
                .prune_kept_events(Utc::now() + Duration::hours(1))
                .unwrap()
                > 0
        );
        assert_eq!(store.events_after(&run.id, 0, 50).unwrap().len(), 0);

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
