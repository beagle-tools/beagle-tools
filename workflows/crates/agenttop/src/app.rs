//! Application state: a point-in-time snapshot of the store (agents and
//! run history) plus the jobs screens' UI state.

use std::time::{Duration, Instant};

use agent_core::state::{RunFilter, Store};
use agent_core::types::{AgentSpec, Event, Run, RunState};
use agent_core::CoreError;

use crate::format::clamp_selection;
use crate::tmux;

/// Runs read for the jobs screens. Every state, every agent — the run
/// history of a job is its chains, and a chain is many runs.
const HISTORY_LIMIT: u32 = 600;
pub(crate) const LOG_EVENT_LIMIT: u32 = 1000;
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
    /// agentd's own status, from its heartbeat; `None` before the first
    /// refresh. Anything but running puts a red bar on every screen.
    pub daemon: Option<agent_core::daemon::DaemonStatus>,
    /// What the daemon last complained about during retention — typically
    /// `gh` being unreachable, which means write-ups are piling up rather
    /// than being pruned. Worth saying out loud, not just logging.
    pub retention_problem: Option<String>,
    pub agent_count: usize,
    /// Declared agents, ordered by project then name — the DAG view's nodes.
    pub agents: Vec<AgentSpec>,
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
    /// `r` on the jobs page: only jobs with a run live. `a` clears it.
    pub job_running_only: bool,
    /// `~/.config/agentd/ui.yaml`, re-read every refresh: the projects
    /// (demo, usually) the jobs page leaves out, ticket links, and where
    /// investigation write-ups land.
    pub view: agent_core::ui::UiConfig,
    /// That file failing to parse — shown in the header, hiding nothing.
    pub view_error: Option<String>,
    /// `H`: show the hidden projects anyway, until pressed again.
    pub show_hidden: bool,
    /// The job drilled into with enter: `(project, name)`. `None` is the
    /// jobs list; `Some` is that job's runs screen.
    pub job_open: Option<(String, String)>,
    /// `A` on the jobs list: the runs screen for EVERY job at once — each
    /// chain under its partition, newest first, refreshing live.
    pub all_runs: bool,
    /// `r` on that page: only chains with a step still pending or running.
    pub all_running_only: bool,
    /// `W` on a runs screen: the cursor rides the newest step at the top
    /// as runs arrive. j/k turns it off.
    pub follow_top: bool,
    /// Selected step on the runs screen, as an index into
    /// [`App::job_steps`] — steps only, so the cursor never stops on a
    /// run's partition header.
    pub job_step_selected: usize,
    /// The last few hundred runs of every agent, for the jobs screens.
    pub history: Vec<Run>,
    /// The draft PR of the step selected on the runs screen, cached as
    /// `(run id, PR)` and recomputed only when the selection or the store
    /// moves — it is a store read plus a scan of the run's events, too
    /// much to repeat on every frame.
    pub step_pr: Option<(String, Option<String>)>,
    pub show_help: bool,
    /// When set, the jobs screens are replaced by this run's event log.
    pub log_view: Option<LogView>,
    status: Option<(String, Instant)>,
    /// The `R` confirmation pane, while it is open (`confirm.rs`).
    pub confirm: Option<crate::confirm::Confirm>,
}

impl App {
    pub fn new() -> Self {
        Self {
            // Open on the JOBS page, not the run firehose. "What can I
            // run, and is any of it going?" is the question you start
            // with; a flat list of every run of every step is the one
            // you drill down to. `d` still toggles back to it.
            ..Self::default()
        }
    }

    /// Re-query the store. Any failure lands in `store_error` (shown in the
    /// jobs header) rather than tearing the TUI down.
    pub fn refresh(&mut self) {
        self.daemon = Some(agent_core::daemon::status());
        match agent_core::ui::UiConfig::try_load() {
            Ok(view) => {
                self.view = view;
                self.view_error = None;
            }
            Err(e) => {
                self.view = agent_core::ui::UiConfig::default();
                self.view_error = Some(format!("ui.yaml: {e}"));
            }
        }
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
            Some(store) => Snapshot::load(store),
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
                // The cursor follows its run, not its row: on a live page
                // new chains arrive at the top and would otherwise slide
                // a different step under the cursor every second.
                let keep = self.dag_selected_run().map(|r| r.id.clone());
                self.history = snap.history;
                self.dag_selected = clamp_selection(self.dag_selected, self.jobs().len());
                if self.follow_top {
                    self.job_step_selected = 0;
                } else if let Some(at) =
                    keep.and_then(|id| self.job_steps().iter().position(|r| r.id == id))
                {
                    self.job_step_selected = at;
                }
                self.job_step_selected =
                    clamp_selection(self.job_step_selected, self.job_steps().len());
                self.update_step_pr();
            }
            Err(e) => self.store_error = Some(format!("store query failed: {e}")),
        }
        self.refresh_log_view();
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

    /// tmux session to attach to, when the selected run is running and has one.
    pub fn attach_target(&self) -> Option<String> {
        let run = self.dag_selected_run()?;
        if run.state == RunState::Running {
            run.tmux_session.clone()
        } else {
            None
        }
    }

    /// Kill the selected running run: tmux session (best effort), then store
    /// state + event, then refresh so the row moves to RECENT.
    pub fn kill_selected(&mut self) {
        let Some(run) = self.dag_selected_run().cloned() else {
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

/// One consistent read of everything the jobs screens show.
struct Snapshot {
    agents: Vec<AgentSpec>,
    history: Vec<Run>,
}

impl Snapshot {
    fn load(store: &Store) -> Result<Self, CoreError> {
        let agents = store.list_agents()?;
        let history = store.list_runs(&RunFilter {
            limit: Some(HISTORY_LIMIT),
            ..RunFilter::default()
        })?;
        Ok(Self { agents, history })
    }
}

#[cfg(test)]
// Tests should fail loudly on a broken fixture; unwrapping is the clearest way.
#[allow(clippy::unwrap_used)]
#[path = "tests/app.rs"]
mod tests;
