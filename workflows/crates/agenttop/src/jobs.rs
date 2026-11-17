//! The jobs screens' model: which jobs exist (narrowed by `/`), the flow
//! drawn under each, and — once one is opened — its runs and their steps.
//!
//! Drawing lives in [`crate::dag`]. This is the same inherent impl on
//! [`App`], split out so selection, search and drawing each stay small.
//!
//! Every list here is built fresh from `history` on each call rather than
//! cached, so the row the cursor is on and the row on screen are computed
//! by the same function from the same data and cannot disagree.

use agent_core::lineage::{Lineage, SubtreeCost};
use agent_core::types::{AgentSpec, Run};
use crossterm::event::KeyCode;

use crate::app::App;
use crate::format::clamp_selection;

/// One job on the landing list.
pub struct JobEntry<'a> {
    pub spec: &'a AgentSpec,
    /// The job's flow, entry point first: `(agent, depth)`.
    pub tree: Vec<(String, usize)>,
    /// Steps in flight right now, across every chain of this job.
    pub running: Vec<String>,
    /// The newest run of the entry point itself.
    pub last: Option<&'a Run>,
}

/// One line of an opened job's runs screen.
pub enum RunRow<'a> {
    /// The partition that opens one run of the job.
    Header { root: &'a Run, cost: SubtreeCost },
    /// One step of that run, at its depth in the chain.
    Step { run: &'a Run, depth: usize },
}

impl App {
    /// Every job, grouped by project and alphabetical within it, narrowed
    /// by the search term. Stable positions: a list you scroll should not
    /// reshuffle because something finished.
    /// How many jobs `ui.yaml` keeps off the page (0 while `H` shows them).
    pub fn hidden_job_count(&self) -> usize {
        if self.show_hidden {
            return 0;
        }
        agent_core::pipelines::pipelines(&self.agents, &self.history)
            .iter()
            .filter(|p| self.view.hides(&p.entry.project))
            .count()
    }

    pub fn jobs(&self) -> Vec<JobEntry<'_>> {
        let needle = self.job_search.to_lowercase();
        let mut out: Vec<JobEntry<'_>> =
            agent_core::pipelines::pipelines(&self.agents, &self.history)
                .into_iter()
                .filter(|p| {
                    needle.is_empty()
                        || p.entry.name.to_lowercase().contains(&needle)
                        || p.entry.project.to_lowercase().contains(&needle)
                })
                .filter(|p| self.show_hidden || !self.view.hides(&p.entry.project))
                .filter(|p| !self.job_running_only || !p.active.is_empty())
                .map(|p| JobEntry {
                    tree: agent_core::pipelines::flow_tree(
                        &p.entry.project,
                        &p.entry.name,
                        &self.history,
                        &self.agents,
                    ),
                    running: p
                        .active_steps()
                        .into_iter()
                        .map(ToOwned::to_owned)
                        .collect(),
                    last: p.last,
                    spec: p.entry,
                })
                .collect();
        out.sort_by(|a, b| {
            a.spec
                .project
                .cmp(&b.spec.project)
                .then_with(|| a.spec.name.cmp(&b.spec.name))
        });
        out
    }

    pub fn selected_job(&self) -> Option<JobEntry<'_>> {
        self.jobs().into_iter().nth(self.dag_selected)
    }

    /// The agent whose manifest `e` edits and the CONFIG pane shows: the
    /// opened job on the runs screen, otherwise the job under the cursor.
    pub fn dag_selected_agent(&self) -> Option<&AgentSpec> {
        if self.all_runs {
            // The job the selected step belongs to — its chain's root.
            let root = self.root_of(self.dag_selected_run()?);
            return self
                .agents
                .iter()
                .find(|a| a.project == root.project && a.name == root.agent);
        }
        if let Some((project, name)) = &self.job_open {
            return self
                .agents
                .iter()
                .find(|a| &a.project == project && &a.name == name);
        }
        self.selected_job().map(|j| j.spec)
    }

    /// The opened job's runs, newest first, each a header followed by its
    /// steps in chain order.
    pub fn job_run_rows(&self) -> Vec<RunRow<'_>> {
        let job = self.job_open.as_ref();
        if job.is_none() && !self.all_runs {
            return Vec::new();
        }
        let lineage = Lineage::build(&self.history);
        let mut out = Vec::new();
        for root in lineage.roots() {
            let wanted = match job {
                Some((project, name)) => &root.project == project && &root.agent == name,
                // Every job's chains — less the projects ui.yaml hides,
                // and, with `r`, less the chains that have finished.
                None => {
                    (self.show_hidden || !self.view.hides(&root.project))
                        && (!self.all_running_only
                            || lineage
                                .rows(Some(&root.id))
                                .iter()
                                .any(|(r, _)| !r.state.is_terminal()))
                }
            };
            if !wanted {
                continue;
            }
            out.push(RunRow::Header {
                root,
                cost: lineage.subtree_cost(root),
            });
            for (run, depth) in lineage.rows(Some(&root.id)) {
                out.push(RunRow::Step { run, depth });
            }
        }
        out
    }

    /// The selectable rows of the runs screen — steps, never headers.
    pub fn job_steps(&self) -> Vec<&Run> {
        self.job_run_rows()
            .into_iter()
            .filter_map(|row| match row {
                RunRow::Step { run, .. } => Some(run),
                RunRow::Header { .. } => None,
            })
            .collect()
    }

    /// The step under the cursor on the runs screen. Also what the `m`
    /// metadata overlay shows while on the jobs screens.
    pub fn dag_selected_run(&self) -> Option<&Run> {
        if !self.runs_open() {
            return None;
        }
        self.job_steps().get(self.job_step_selected).copied()
    }

    /// True on a runs screen — one job's (`enter`) or every job's (`A`).
    pub fn runs_open(&self) -> bool {
        self.job_open.is_some() || self.all_runs
    }

    /// The first run of `run`'s chain.
    pub fn root_of<'a>(&'a self, run: &'a Run) -> &'a Run {
        let mut root = run;
        for _ in 0..32 {
            match root
                .parent_run_id
                .as_deref()
                .and_then(|p| self.history.iter().find(|r| r.id == p))
            {
                Some(parent) => root = parent,
                None => break,
            }
        }
        root
    }

    /// `A` on the jobs list: every job's runs on one live page.
    pub fn open_all_runs(&mut self) {
        self.all_runs = true;
        self.job_open = None;
        self.job_step_selected = 0;
        self.update_step_pr();
    }

    /// j/k: job to job on the list, step to step inside a job.
    pub fn dag_move(&mut self, down: bool) {
        let (len, at) = if self.runs_open() {
            (self.job_steps().len(), self.job_step_selected)
        } else {
            (self.jobs().len(), self.dag_selected)
        };
        let next = if down { at + 1 } else { at.saturating_sub(1) };
        let next = clamp_selection(next, len);
        if self.runs_open() {
            // Moving means you want to look at something: stop following.
            self.follow_top = false;
            self.job_step_selected = next;
            self.update_step_pr();
        } else {
            self.dag_selected = next;
        }
    }

    /// Refresh the cached draft PR for the selected step — see `step_pr`.
    pub fn update_step_pr(&mut self) {
        let Some(id) = self.dag_selected_run().map(|r| r.id.clone()) else {
            self.step_pr = None;
            return;
        };
        let pr = self
            .with_store(|s| Ok(agent_core::links::run_pr(s, &id)))
            .ok()
            .flatten();
        self.step_pr = Some((id, pr));
    }

    /// The write-up worth reading for the selected step, most specific
    /// first: an investigation's OUTPUT.md, then the step's own write-up,
    /// then the whole chain's handover. `None` when the step has none —
    /// most stages do not write one, and old ones are swept after a day.
    pub fn step_writeup(&self) -> Option<std::path::PathBuf> {
        let run = self.dag_selected_run()?;
        // `investigations_dir` in ui.yaml; unset means no investigations.
        if let Some(dir) = self.view.investigations_dir() {
            let investigation = dir.join(&run.id).join("OUTPUT.md");
            if investigation.is_file() {
                return Some(investigation);
            }
        }
        // A step's own short summary in its data dir (kept a week): what a
        // run like a notes-refresh step did, in a screen.
        if let Ok(data) = agent_core::paths::data_dir() {
            let summary = data.join(&run.id).join("summary.md");
            if summary.is_file() {
                return Some(summary);
            }
        }
        let own = crate::detail::summary_path(&run.id).filter(|p| p.is_file());
        if own.is_some() {
            return own;
        }
        // The chain's handover is keyed by the root — walk up to it.
        let by_id: std::collections::HashMap<&str, &Run> =
            self.history.iter().map(|r| (r.id.as_str(), r)).collect();
        let mut root = run;
        for _ in 0..32 {
            match root.parent_run_id.as_deref().and_then(|p| by_id.get(p)) {
                Some(parent) => root = parent,
                None => break,
            }
        }
        crate::detail::summary_path(&root.id).filter(|p| p.is_file())
    }

    /// The selected step's draft PR, if it has one.
    pub fn selected_step_pr(&self) -> Option<&str> {
        let (id, pr) = self.step_pr.as_ref()?;
        (self.dag_selected_run().map(|r| r.id.as_str()) == Some(id.as_str()))
            .then_some(pr.as_deref())
            .flatten()
    }

    /// enter on the list: open the job under the cursor.
    pub fn open_job(&mut self) {
        let Some(key) = self
            .selected_job()
            .map(|j| (j.spec.project.clone(), j.spec.name.clone()))
        else {
            return;
        };
        self.job_open = Some(key);
        self.job_step_selected = 0;
        self.update_step_pr();
    }

    /// `W`: follow the newest step, or stop following.
    pub fn toggle_follow(&mut self) {
        self.follow_top = !self.follow_top;
        if self.follow_top {
            self.job_step_selected = 0;
            self.update_step_pr();
        }
    }

    pub fn close_job(&mut self) {
        self.job_open = None;
        self.all_runs = false;
        self.follow_top = false;
        self.job_step_selected = 0;
    }

    /// True while a job search is being typed: keys are text, not moves.
    pub fn job_search_typing(&self) -> bool {
        !self.runs_open() && self.job_search_input
    }

    pub fn job_search_begin(&mut self) {
        self.job_search_input = true;
        self.job_search.clear();
        self.dag_selected = 0;
    }

    /// One keystroke of the search term. Enter keeps the filter and goes
    /// back to moving; esc drops it.
    pub fn job_search_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char(c) => {
                self.job_search.push(c);
                self.dag_selected = 0;
            }
            KeyCode::Backspace => {
                self.job_search.pop();
                self.dag_selected = 0;
            }
            KeyCode::Enter => self.job_search_input = false,
            KeyCode::Esc => {
                self.job_search_input = false;
                self.job_search.clear();
                self.dag_selected = 0;
            }
            _ => {}
        }
    }
}
