//! Application state: a point-in-time snapshot of the store plus UI state.
//!
//! Selection is one logical list spanning the ACTIVE pane (running + pending
//! runs) followed by the RECENT pane (terminal runs) — j/k walk through both.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use agent_core::state::{RunFilter, Store};
use agent_core::types::{AgentSpec, Event, Run, RunState};
use agent_core::CoreError;

use crate::format::{clamp_selection, payload_snippet};
use crate::tmux;

/// Shown when the store has no runs (or does not exist yet).
pub const EMPTY_HINT: &str = "no runs yet — is agentd running? \
     try: agentctl apply && agentctl run <project>/<agent>";

const RECENT_LIMIT: u32 = 30;
const EVENT_LIMIT: u32 = 100;
/// Runs read for the jobs screens. Every state, every agent — the run
/// history of a job is its chains, and a chain is many runs.
const HISTORY_LIMIT: u32 = 600;
pub(crate) const LOG_EVENT_LIMIT: u32 = 1000;
const ACTIVITY_SNIPPET_CHARS: usize = 80;
const STATUS_TTL: Duration = Duration::from_secs(4);

/// Full-screen event log for one run (opened with `l`, or enter on a
/// finished run). Refreshed incrementally by [`App::refresh`].
#[derive(Debug)]
pub struct LogView {
    pub run: Run,
    pub events: Vec<Event>,
    /// Scroll offset in lines from the tail; 0 = follow new events.
    pub scroll_from_bottom: usize, // search moves this to bring a hit on screen
    /// Horizontal pan. 0 = wrapped; right unwraps and shifts left.
    pub scroll_x: u16,
    /// Active search term, lowercased. `None` until `/` is pressed.
    pub search: Option<String>,
    /// True while typing a search; keystrokes go to the term, not the view.
    pub search_input: bool,
    /// Which hit `n`/`N` is on, as (1-based position, total). Tracked
    /// explicitly rather than inferred from the scroll offset: two hits
    /// inside one screenful would otherwise make `n` look like it did
    /// nothing, because the viewport barely moves.
    pub search_at: Option<(usize, usize)>,
    /// Line index of the current hit, so it can be highlighted differently
    /// from the others.
    pub search_line: Option<usize>,
}

/// All state the UI renders from, refreshed from the store at most once a
/// second by the main loop.
#[derive(Debug, Default)]
// Each bool is an independent overlay/mode toggle that the key handler reads
// directly; folding them into an enum would force a single "current screen"
// that the help overlay and the detail overlay both have to escape from.
#[allow(clippy::struct_excessive_bools)]
pub struct App {
    store: Option<Store>,
    /// Last store open/query failure, shown instead of crashing.
    pub store_error: Option<String>,
    /// What the daemon last complained about during retention — typically
    /// `gh` being unreachable, which means write-ups are piling up rather
    /// than being pruned. Worth saying out loud, not just logging.
    pub retention_problem: Option<String>,
    pub agent_count: usize,
    /// Declared agents, ordered by project then name — the DAG view's nodes.
    pub agents: Vec<AgentSpec>,
    /// When true, the dashboard is replaced by the DAG page (`d`).
    pub show_dag: bool,
    /// The expanded run-detail overlay (`m`), ~85% of the screen.
    pub show_run_detail: bool,
    /// Which link is selected inside that overlay.
    pub detail_selected: usize,
    /// Selected job on the jobs screen, as an index into
    /// [`App::jobs`] — which is already filtered by the search, so the
    /// cursor only ever lands on a job that is on screen.
    pub dag_selected: usize,
    /// The job-name filter typed after `/`.
    pub job_search: String,
    /// True while that filter is being typed; keys are text, not moves.
    pub job_search_input: bool,
    /// The job drilled into with enter: `(project, name)`. `None` is the
    /// jobs list; `Some` is that job's runs screen.
    pub job_open: Option<(String, String)>,
    /// Selected step on the runs screen, as an index into
    /// [`App::job_steps`] — steps only, so the cursor never stops on a
    /// run's partition header.
    pub job_step_selected: usize,
    /// The last few hundred runs of every agent, for the jobs screens.
    /// `recent` is capped at 30 for the dashboard, which with pr-watch
    /// ticking every 15 minutes is only the last handful of chains — not
    /// "every previous run of this job".
    pub history: Vec<Run>,
    /// RECENT pane state filter, cycled with `f`: 0 all, 1 succeeded, 2 failed.
    pub recent_filter: u8,
    /// Running runs first, then pending — the ACTIVE pane.
    pub active: Vec<Run>,
    /// Terminal runs, newest first — the RECENT pane.
    pub recent: Vec<Run>,
    /// Recent events across all runs, oldest first.
    pub events: Vec<Event>,
    /// run id -> latest "kind snippet" line for the ACTIVE pane.
    pub last_activity: HashMap<String, String>,
    /// Index into the logical active+recent list.
    pub selected: usize,
    pub show_help: bool,
    /// When set, the dashboard is replaced by this run's event log.
    pub log_view: Option<LogView>,
    status: Option<(String, Instant)>,
}

impl App {
    pub fn new() -> Self {
        Self {
            // Open on the JOBS page, not the run firehose. "What can I
            // run, and is any of it going?" is the question you start
            // with; a flat list of every run of every step is the one
            // you drill down to. `d` still toggles back to it.
            show_dag: true,
            ..Self::default()
        }
    }

    /// Re-query the store. Any failure lands in `store_error` (rendered in
    /// the ACTIVE pane) rather than tearing the dashboard down.
    pub fn refresh(&mut self) {
        if self.store.is_none() {
            match Store::open_default() {
                Ok(store) => self.store = Some(store),
                Err(e) => {
                    self.store_error = Some(format!("cannot open state db: {e}"));
                    return;
                }
            }
        }
        let snapshot = match &self.store {
            Some(store) => Snapshot::load(store, self.recent_filter_states()),
            None => return,
        };
        match snapshot {
            Ok(snap) => {
                self.store_error = None;
                self.retention_problem = self
                    .store
                    .as_ref()
                    .and_then(|s| s.get_meta(crate::RETENTION_PROBLEM_KEY).ok().flatten());
                self.agent_count = snap.agents.len();
                // Reading order: each project's JOBS, with the steps
                // they run nested under them. The DAGS pane selects by
                // index, so the list and the display must be the same
                // order — sorting here is what keeps the row you are on
                // and the agent you get the same thing.
                self.agents = agent_core::pipelines::display_order(&snap.agents, &snap.history);
                self.history = snap.history;
                self.last_activity = build_activity(&snap.events);
                self.active = snap.active;
                self.recent = snap.recent;
                self.events = snap.events;
                self.selected = clamp_selection(self.selected, self.row_count());
                self.dag_selected = clamp_selection(self.dag_selected, self.jobs().len());
                self.job_step_selected =
                    clamp_selection(self.job_step_selected, self.job_steps().len());
            }
            Err(e) => self.store_error = Some(format!("store query failed: {e}")),
        }
        self.refresh_log_view();
    }

    /// Cycle the RECENT filter (all → succeeded → failed) and re-query.
    pub fn cycle_recent_filter(&mut self) {
        self.recent_filter = (self.recent_filter + 1) % 3;
        self.refresh();
    }

    fn recent_filter_states(&self) -> Option<Vec<RunState>> {
        match self.recent_filter {
            1 => Some(vec![RunState::Succeeded]),
            2 => Some(vec![RunState::Failed, RunState::TimedOut, RunState::Killed]),
            _ => None,
        }
    }

    /// Suffix for the RECENT pane title showing the active filter.
    pub fn recent_filter_label(&self) -> &'static str {
        match self.recent_filter {
            1 => " · succeeded",
            2 => " · failed",
            _ => "",
        }
    }

    /// Pull events written since the last refresh into the open log view and
    /// keep its run header (state, timings) current.
    fn refresh_log_view(&mut self) {
        let Some(view) = &self.log_view else {
            return;
        };
        let run_id = view.run.id.clone();
        let last_id = view.events.last().map_or(0, |e| e.id);
        let fresh = self.with_store(|s| s.events_after(&run_id, last_id, LOG_EVENT_LIMIT));
        let run = self.with_store(|s| s.get_run(&run_id));
        let Some(view) = &mut self.log_view else {
            return;
        };
        if let Ok(events) = fresh {
            view.events.extend(events);
        }
        if let Ok(Some(run)) = run {
            view.run = run;
        }
    }

    /// Scroll the log view. Scrolling up detaches from the tail; scrolling
    /// back to the bottom re-follows. `max_scroll` is the display-line bound
    /// computed by the caller (only the renderer knows wrap widths).
    pub fn scroll_logs(&mut self, up: bool, max_scroll: usize) {
        let Some(view) = &mut self.log_view else {
            return;
        };
        if up {
            view.scroll_from_bottom = (view.scroll_from_bottom + 1).min(max_scroll);
        } else {
            view.scroll_from_bottom = view.scroll_from_bottom.saturating_sub(1);
        }
    }

    /// True when the store holds nothing to show at all.
    pub fn is_empty(&self) -> bool {
        self.active.is_empty() && self.recent.is_empty() && self.events.is_empty()
    }

    /// tmux session to attach to, when the selected run is running and has one.
    pub fn attach_target(&self) -> Option<String> {
        let run = self.selected_run()?;
        if run.state == RunState::Running {
            run.tmux_session.clone()
        } else {
            None
        }
    }

    /// Kill the selected running run: tmux session (best effort), then store
    /// state + event, then refresh so the row moves to RECENT.
    pub fn kill_selected(&mut self) {
        let Some(run) = self.selected_run().cloned() else {
            self.set_status("nothing selected");
            return;
        };
        if run.state != RunState::Running {
            self.set_status("only running runs can be killed");
            return;
        }
        if let Some(session) = &run.tmux_session {
            tmux::kill_session(session);
        }
        let result = self.with_store(|store| {
            store.finish_run(&run.id, RunState::Killed, None)?;
            store.insert_event(&run.id, "state", Some("killed via agenttop"))
        });
        match result {
            Ok(()) => self.set_status(&format!("killed {}", run.id)),
            Err(e) => self.set_status(&format!("kill failed: {e}")),
        }
        self.refresh();
    }

    pub fn set_status(&mut self, message: &str) {
        self.status = Some((message.to_owned(), Instant::now()));
    }

    /// Transient status line, visible for a few seconds after an action.
    pub fn status_message(&self) -> Option<&str> {
        self.status
            .as_ref()
            .filter(|(_, at)| at.elapsed() < STATUS_TTL)
            .map(|(msg, _)| msg.as_str())
    }

    pub(crate) fn with_store<T>(
        &self,
        f: impl FnOnce(&Store) -> Result<T, CoreError>,
    ) -> Result<T, CoreError> {
        match &self.store {
            Some(store) => f(store),
            None => Err(CoreError::Other("state db is not open".to_owned())),
        }
    }
}

/// One consistent read of everything the dashboard shows.
struct Snapshot {
    agents: Vec<AgentSpec>,
    active: Vec<Run>,
    recent: Vec<Run>,
    events: Vec<Event>,
    history: Vec<Run>,
}

impl Snapshot {
    fn load(store: &Store, recent_states: Option<Vec<RunState>>) -> Result<Self, CoreError> {
        let agents = store.list_agents()?;
        let mut active = store.running_runs()?;
        active.extend(store.pending_runs()?);
        let recent = store.list_runs(&RunFilter {
            states: Some(recent_states.unwrap_or_else(|| {
                vec![
                    RunState::Succeeded,
                    RunState::Failed,
                    RunState::TimedOut,
                    RunState::Killed,
                ]
            })),
            limit: Some(RECENT_LIMIT),
            ..RunFilter::default()
        })?;
        // recent_events returns newest first; the feed renders oldest first
        // with the newest at the bottom.
        let mut events = store.recent_events(EVENT_LIMIT)?;
        events.reverse();
        let history = store.list_runs(&RunFilter {
            limit: Some(HISTORY_LIMIT),
            ..RunFilter::default()
        })?;
        Ok(Self {
            agents,
            active,
            recent,
            events,
            history,
        })
    }
}

/// Latest activity line per run. Events arrive oldest first, so later
/// inserts overwrite earlier ones and the newest wins.
fn build_activity(events: &[Event]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for event in events {
        let snippet = payload_snippet(event.payload.as_deref(), ACTIVITY_SNIPPET_CHARS);
        let line = if snippet.is_empty() {
            event.kind.clone()
        } else {
            format!("{} {snippet}", event.kind)
        };
        map.insert(event.run_id.clone(), line);
    }
    map
}

#[cfg(test)]
// Tests should fail loudly on a broken fixture; unwrapping is the clearest way.
#[allow(clippy::unwrap_used)]
#[path = "tests/app.rs"]
mod tests;
