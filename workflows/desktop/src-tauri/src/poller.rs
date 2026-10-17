//! The bridge between a file that changes and a UI that should not poll.
//!
//! agentd's store is a SQLite file with no change notification, so
//! *somebody* has to poll it. Doing that in the frontend would mean an
//! IPC round trip and a React re-render every tick whether or not
//! anything moved. Doing it here means one cheap query on a background
//! thread, and an event only when the answer actually changed — so the
//! webview re-renders when a run changes state, not on a timer.
//!
//! The thread opens its OWN `Store`. `agent_core` documents one store per
//! thread, and sharing the app's would mean the poller took that lock
//! every two seconds, competing with whatever the user is doing.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::Duration;

use agent_core::state::{RunFilter, Store};
use tauri::{AppHandle, Emitter};

/// Fast enough that a state change feels immediate, slow enough that it
/// costs nothing. agenttop refreshes on the same order.
const INTERVAL: Duration = Duration::from_secs(2);

pub const RUNS_UPDATED: &str = "agentdesk://runs-updated";
pub const STORE_PROBLEM: &str = "agentdesk://store-problem";

pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || {
        let store = match Store::open_default() {
            Ok(store) => store,
            Err(e) => {
                // Without the store there is no app. Say so once, in the
                // UI, rather than leaving an empty window and a log line.
                let _ = app.emit(
                    STORE_PROBLEM,
                    format!("could not open the agentd store: {e}. Is agentd installed?"),
                );
                return;
            }
        };
        let mut last: Option<u64> = None;
        let mut reported_problem = false;
        loop {
            match fingerprint(&store) {
                Ok(current) => {
                    if reported_problem {
                        let _ = app.emit(STORE_PROBLEM, "");
                        reported_problem = false;
                    }
                    if last != Some(current) {
                        last = Some(current);
                        let _ = app.emit(RUNS_UPDATED, ());
                    }
                }
                Err(e) => {
                    // Report once per outage, not once per tick — a
                    // locked database during a daemon write is normal and
                    // transient, and a toast every two seconds is worse
                    // than the problem.
                    if !reported_problem {
                        reported_problem = true;
                        let _ = app.emit(STORE_PROBLEM, format!("store read failed: {e}"));
                    }
                }
            }
            std::thread::sleep(INTERVAL);
        }
    });
}

/// A hash of everything the runs table displays. Comparing this instead of
/// the rows themselves means a tick costs one query and one integer
/// compare, and an idle machine emits nothing at all.
fn fingerprint(store: &Store) -> Result<u64, agent_core::error::CoreError> {
    let runs = store.list_runs(&RunFilter {
        project: None,
        agent: None,
        states: None,
        limit: Some(200),
    })?;
    let mut hasher = DefaultHasher::new();
    runs.len().hash(&mut hasher);
    for run in &runs {
        run.id.hash(&mut hasher);
        run.state.to_string().hash(&mut hasher);
        run.exit_code.hash(&mut hasher);
        run.finished_at.hash(&mut hasher);
        // Cost lands after the run finishes, so it is a real change the
        // table must pick up. f64 has no Hash; its bits do.
        run.cost_usd.map(f64::to_bits).hash(&mut hasher);
        run.num_turns.hash(&mut hasher);
    }
    Ok(hasher.finish())
}
