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
                .map(|p| JobEntry {
                    tree: agent_core::pipelines::flow_tree(
                        &p.entry.project,
                        &p.entry.name,
                        &self.history,
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
        let Some((project, name)) = &self.job_open else {
            return Vec::new();
        };
        let lineage = Lineage::build(&self.history);
        let mut out = Vec::new();
        for root in lineage.roots() {
            if &root.project != project || &root.agent != name {
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
        self.job_open.as_ref()?;
        self.job_steps().get(self.job_step_selected).copied()
    }

    /// j/k: job to job on the list, step to step inside a job.
    pub fn dag_move(&mut self, down: bool) {
        let (len, at) = if self.job_open.is_some() {
            (self.job_steps().len(), self.job_step_selected)
        } else {
            (self.jobs().len(), self.dag_selected)
        };
        let next = if down { at + 1 } else { at.saturating_sub(1) };
        let next = clamp_selection(next, len);
        if self.job_open.is_some() {
            self.job_step_selected = next;
        } else {
            self.dag_selected = next;
        }
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
    }

    pub fn close_job(&mut self) {
        self.job_open = None;
        self.job_step_selected = 0;
    }

    /// True while a job search is being typed: keys are text, not moves.
    pub fn job_search_typing(&self) -> bool {
        self.show_dag && self.job_open.is_none() && self.job_search_input
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
