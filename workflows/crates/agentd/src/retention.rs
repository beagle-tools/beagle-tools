//! Log retention: delete the log file and event feed of runs that finished
//! more than the configured window ago (rows stay as history), and — for
//! agents opting in via `noop_retention_minutes` — delete quiet poller
//! ticks outright.
//!
//! Data dirs (the handover between a chain's steps) are kept far longer
//! than logs — [`agent_core::resume::DATA_RETENTION_HOURS`] — because they
//! are what lets a failed step be resumed in place days later, and they are
//! a few KB each.

use std::path::{Path, PathBuf};

use agent_core::state::{RunFilter, Store};
use agent_core::types::{AgentSpec, Run, RunState};
use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use tracing::{info, warn};

/// One retention pass. `retention_hours` of 0 disables log pruning entirely.
/// `now` is injected so tests control the clock.
/// Delete run write-ups older than this. Deliberately longer than the log
/// window: a summary exists to be read the next morning, by which time the
/// logs are long gone.
pub(crate) const SUMMARY_RETENTION_HOURS: i64 = 24;

/// Meta key the daemon writes when it cannot reach GitHub, so agenttop can
/// say so rather than the problem living only in a log nobody opens.
pub const RETENTION_PROBLEM_KEY: &str = "retention_problem";

/// The directories a retention pass may delete from — passed in, never
/// looked up inside the pass.
///
/// They used to be looked up (`paths::state_dir()`), which meant a TEST
/// running a pass against a throwaway store still swept the REAL
/// `summaries/`: none of the machine's real runs exist in the test store,
/// so every real write-up looked orphaned and was deleted — silently, since
/// tests install no logger. Every `cargo test` wiped every write-up on the
/// machine, minutes after the report step wrote it.
/// A pass now touches only the directories it is handed.
#[derive(Debug, Clone)]
pub struct Dirs {
    pub summaries: PathBuf,
    pub data: PathBuf,
}

impl Dirs {
    /// The daemon's real directories. Only `main` calls this.
    pub fn real() -> Result<Self> {
        Ok(Self {
            summaries: agent_core::paths::state_dir()?.join("summaries"),
            data: agent_core::paths::data_dir()?,
        })
    }
}

pub fn tick(
    store: &Store,
    specs: &[AgentSpec],
    now: DateTime<Utc>,
    retention_hours: u64,
    dirs: &Dirs,
) -> Result<()> {
    crate::summaries::prune_summaries(store, &dirs.summaries, now);
    prune_noop_runs(store, specs, now, &dirs.data)?;
    prune_data_dirs(store, now, &dirs.data);
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
        // Not the data dir: that outlives the log (see prune_data_dirs).
        info!(run = %run.id, "pruned logs + events (retention window passed)");
    }
    Ok(())
}

/// Delete no-op runs — succeeded, or timed out (e.g. the machine slept
/// mid-poll and the monitor caught up on wake), without writing an
/// announcement — once they're older than the agent's
/// `noop_retention_minutes`. Failed/killed runs are always kept, and the
/// newest run of all is kept so "last run" status stays visible.
fn prune_noop_runs(
    store: &Store,
    specs: &[AgentSpec],
    now: DateTime<Utc>,
    data: &Path,
) -> Result<()> {
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
            if !expired || announced(run, data) {
                continue;
            }
            if let Some(log) = &run.log_path {
                let _ = std::fs::remove_file(log);
            }
            let _ = std::fs::remove_dir_all(data.join(&run.id));
            store.delete_run(&run.id)?;
            info!(run = %run.id, "deleted no-op run (noop_retention_minutes)");
        }
    }
    Ok(())
}

/// Delete data dirs of runs that finished more than a week ago, and of
/// runs whose row is gone, once the dir itself is that old. A pending or
/// running run's dir is never touched. Best-effort: a dir that will not go
/// is retried next pass.
fn prune_data_dirs(store: &Store, now: DateTime<Utc>, data: &Path) {
    let Some(cutoff) =
        Duration::try_hours(agent_core::resume::DATA_RETENTION_HOURS).map(|w| now - w)
    else {
        return;
    };
    // The error evidence prune_run_artifacts kept goes with the data dir.
    if let Err(e) = store.prune_kept_events(cutoff) {
        warn!(error = %e, "could not prune kept events");
    }
    let Ok(entries) = std::fs::read_dir(data) else {
        return;
    };
    for entry in entries.flatten() {
        let Some(id) = entry.file_name().to_str().map(ToOwned::to_owned) else {
            continue;
        };
        let expired = match store.get_run(&id) {
            Ok(Some(run)) => run.finished_at.is_some_and(|t| t < cutoff),
            Ok(None) => entry
                .metadata()
                .and_then(|m| m.modified())
                .is_ok_and(|t| DateTime::<Utc>::from(t) < cutoff),
            Err(_) => false,
        };
        if expired {
            match std::fs::remove_dir_all(entry.path()) {
                Ok(()) => info!(run = %id, "pruned data dir (resume window passed)"),
                Err(e) => warn!(run = %id, error = %e, "could not delete data dir"),
            }
        }
    }
}

/// A run that wrote `$AGENTD_DATA_DIR/notify.txt` did real work — never
/// treated as a no-op.
fn announced(run: &Run, data: &Path) -> bool {
    data.join(&run.id).join("notify.txt").is_file()
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
#[path = "tests/retention.rs"]
mod tests;
