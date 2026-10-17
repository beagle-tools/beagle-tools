//! Pipelines: the jobs you actually start, and the steps they run.
//!
//! The store knows about *agents*, and every stage of a chain is one —
//! `linear` and `linear-implement` sit side by side in the same list.
//! That is right for the daemon and wrong for a person: nobody starts
//! `linear-implement`, and a list of eleven agents hides the three jobs
//! there really are.
//!
//! So a **pipeline** is an entry point plus the steps observed
//! downstream of it. It is derived, not declared, because the chain
//! shape is a runtime fact — `gather.sh` decides what to trigger, and a
//! manifest listing its own steps would go stale the first time that
//! script changed.

use std::collections::{BTreeSet, HashMap};

use crate::lineage::Lineage;

pub use crate::flow::flow_tree;
use crate::types::{AgentSpec, Run, RunState};

/// A high-level job: what you pick, what it is made of, what it is doing.
#[derive(Debug)]
pub struct Pipeline<'a> {
    /// The agent you start: scheduled, or never triggered by another run.
    pub entry: &'a AgentSpec,
    /// Agents seen downstream of this entry point, ordered by how deep
    /// in the chain they run. Empty until the pipeline has run once.
    ///
    /// Depth, not first-observed: the list is read as a sequence, and
    /// first-observed order is arbitrary — it rendered the linear
    /// pipeline as `report → investigate → implement → context`, which
    /// is close to backwards.
    pub steps: Vec<String>,
    /// Runs anywhere in this pipeline that are pending or running.
    pub active: Vec<&'a Run>,
    /// The newest run of the entry point itself, whatever its state.
    pub last: Option<&'a Run>,
}

impl Pipeline<'_> {
    /// Whether any run in the pipeline is pending or running.
    pub fn is_running(&self) -> bool {
        !self.active.is_empty()
    }

    /// The names of the steps currently in flight — what "running" means
    /// concretely. Two chains of the same pipeline can be at different
    /// steps at once, so this is a set, not a single value.
    pub fn active_steps(&self) -> Vec<&str> {
        let mut seen = BTreeSet::new();
        for run in &self.active {
            seen.insert(run.agent.as_str());
        }
        seen.into_iter().collect()
    }

    /// `project/name` of the entry agent — unique per pipeline.
    pub fn key(&self) -> String {
        format!("{}/{}", self.entry.project, self.entry.name)
    }
}

/// Is this agent something a person starts, or a step some other agent
/// triggers?
///
/// Declared when the manifest says so. Otherwise inferred, and the
/// inference is simply "has it ever started a chain": an agent with a
/// root run is one somebody (or cron) began, and an agent whose runs all
/// have parents has only ever been triggered.
///
/// An agent that has never run at all counts as an entry point. Showing
/// a step too early is a cosmetic mistake that fixes itself on the first
/// run; hiding a job that exists is one nobody can see to correct.
pub fn is_entry(spec: &AgentSpec, runs: &[Run]) -> bool {
    if let Some(declared) = spec.entrypoint {
        return declared;
    }
    if spec.schedule.is_some() {
        return true;
    }
    let mut seen_any = false;
    for run in runs {
        if run.project != spec.project || run.agent != spec.name {
            continue;
        }
        seen_any = true;
        if run.parent_run_id.is_none() {
            return true;
        }
    }
    !seen_any
}

/// Group agents into pipelines, newest-activity first.
pub fn pipelines<'a>(specs: &'a [AgentSpec], runs: &'a [Run]) -> Vec<Pipeline<'a>> {
    let by_id: HashMap<&str, &Run> = runs.iter().map(|r| (r.id.as_str(), r)).collect();

    // Which entry point does each run belong to? Walk to the chain root
    // and take its agent — that is the pipeline the run is part of,
    // however deep it sits.
    let mut owner: HashMap<&str, String> = HashMap::new();
    for run in runs {
        let mut cursor = run;
        for _ in 0..32 {
            match cursor.parent_run_id.as_deref().and_then(|p| by_id.get(p)) {
                Some(parent) => cursor = parent,
                None => break,
            }
        }
        owner.insert(
            run.id.as_str(),
            format!("{}/{}", cursor.project, cursor.agent),
        );
    }

    let mut out: Vec<Pipeline<'a>> = Vec::new();
    for spec in specs.iter().filter(|s| is_entry(s, runs)) {
        let key = format!("{}/{}", spec.project, spec.name);
        let mine: Vec<&Run> = runs
            .iter()
            .filter(|r| owner.get(r.id.as_str()).is_some_and(|o| *o == key))
            .collect();
        let sub = Lineage::build(mine.iter().copied());

        // Shallowest first. A step seen at several depths takes the
        // shallowest it has ever run at, so a retry deeper in a chain
        // does not reorder the sequence.
        let mut depth_of: std::collections::BTreeMap<String, usize> =
            std::collections::BTreeMap::new();
        for (run, depth) in sub.rows(None) {
            if run.agent == spec.name {
                continue;
            }
            depth_of
                .entry(run.agent.clone())
                .and_modify(|d| *d = (*d).min(depth))
                .or_insert(depth);
        }
        let mut ordered: Vec<(usize, String)> = depth_of.into_iter().map(|(a, d)| (d, a)).collect();
        ordered.sort();
        let steps: Vec<String> = ordered.into_iter().map(|(_, a)| a).collect();

        out.push(Pipeline {
            entry: spec,
            steps,
            active: mine
                .iter()
                .copied()
                .filter(|r| matches!(r.state, RunState::Pending | RunState::Running))
                .collect(),
            last: mine
                .iter()
                .copied()
                .filter(|r| r.agent == spec.name)
                .max_by_key(|r| r.created_at),
        });
    }

    // Running first — "what is happening now" is the question the list
    // opens with — then by most recent activity, then by name so an
    // unused pipeline still has a stable place.
    out.sort_by(|a, b| {
        b.is_running()
            .cmp(&a.is_running())
            .then_with(|| {
                b.last
                    .map(|r| r.created_at)
                    .cmp(&a.last.map(|r| r.created_at))
            })
            .then_with(|| a.key().cmp(&b.key()))
    });
    out
}

/// Agents in reading order: each project's jobs, and under each job the
/// steps it runs.
///
/// Returned as a flat list because the TUI selects by index — the row
/// you are on has to be the agent you get, and the cheapest way to
/// guarantee that is for the list and the display to be the same order.
/// A step no job claims (nothing has run, so nothing links it) still
/// appears, at the end of its project, rather than vanishing.
pub fn display_order(specs: &[AgentSpec], runs: &[Run]) -> Vec<AgentSpec> {
    let pipes = pipelines(specs, runs);
    let mut out: Vec<AgentSpec> = Vec::new();
    let mut placed: BTreeSet<(String, String)> = BTreeSet::new();

    let mut projects: Vec<&str> = specs.iter().map(|s| s.project.as_str()).collect();
    projects.sort_unstable();
    projects.dedup();

    for project in projects {
        let mut jobs: Vec<&Pipeline<'_>> = pipes
            .iter()
            .filter(|p| p.entry.project == project)
            .collect();
        jobs.sort_by(|a, b| a.entry.name.cmp(&b.entry.name));

        for job in jobs {
            out.push(job.entry.clone());
            placed.insert((project.to_owned(), job.entry.name.clone()));
            for step in &job.steps {
                if let Some(spec) = specs
                    .iter()
                    .find(|s| s.project == project && &s.name == step)
                {
                    if placed.insert((project.to_owned(), step.clone())) {
                        out.push(spec.clone());
                    }
                }
            }
        }
        for spec in specs.iter().filter(|s| s.project == project) {
            if placed.insert((project.to_owned(), spec.name.clone())) {
                out.push(spec.clone());
            }
        }
    }
    out
}

#[cfg(test)]
// Tests should fail loudly on a broken fixture; unwrapping is the clearest way.
#[allow(clippy::unwrap_used)]
#[path = "tests/pipelines.rs"]
mod tests;
