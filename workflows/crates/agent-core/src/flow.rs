//! The shape of a job as it has actually run. Split from `pipelines`,
//! which groups agents into jobs; this draws the edges between them.
//! Re-exported there as `pipelines::flow_tree`.

use std::collections::{BTreeSet, HashMap};

use crate::types::Run;

/// The shape of a job as it has actually run: every agent-to-agent edge
/// observed in any of its chains, unioned, then walked depth-first from
/// the entry point. Rows are `(agent, depth)`; the entry is row 0.
///
/// A union rather than one run's shape, because a job branches — linear
/// goes context → implement on a code ticket and context → investigate on
/// a question — and showing only the latest run would hide whichever
/// branch it did not take. A step reachable two ways (linear-report,
/// under both) appears under each: that is the shape, not duplication.
///
/// Siblings sort by name, so the tree is stable from one refresh to the
/// next. A job that has never run is just its entry point.
pub fn flow_tree(project: &str, entry: &str, runs: &[Run]) -> Vec<(String, usize)> {
    let by_id: HashMap<&str, &Run> = runs.iter().map(|r| (r.id.as_str(), r)).collect();
    let root_agent = |run: &Run| -> String {
        let mut cursor = run;
        for _ in 0..32 {
            match cursor.parent_run_id.as_deref().and_then(|p| by_id.get(p)) {
                Some(parent) => cursor = parent,
                None => break,
            }
        }
        cursor.agent.clone()
    };

    let mut children: HashMap<String, BTreeSet<String>> = HashMap::new();
    for run in runs.iter().filter(|r| r.project == project) {
        let Some(parent) = run.parent_run_id.as_deref().and_then(|p| by_id.get(p)) else {
            continue;
        };
        if root_agent(run) == entry {
            children
                .entry(parent.agent.clone())
                .or_default()
                .insert(run.agent.clone());
        }
    }

    let mut out = Vec::new();
    let mut path: Vec<String> = Vec::new();
    walk_flow(entry, 0, &children, &mut path, &mut out);
    out
}

fn walk_flow(
    agent: &str,
    depth: usize,
    children: &HashMap<String, BTreeSet<String>>,
    path: &mut Vec<String>,
    out: &mut Vec<(String, usize)>,
) {
    out.push((agent.to_owned(), depth));
    // A cycle in the observed edges (an agent that re-triggers an
    // ancestor) would recurse forever; the path guard and the depth cap
    // turn that into a truncated tree instead of a hung terminal.
    if depth >= 12 || path.iter().any(|a| a == agent) {
        return;
    }
    path.push(agent.to_owned());
    for child in children.get(agent).into_iter().flatten() {
        walk_flow(child, depth + 1, children, path, out);
    }
    path.pop();
}

#[cfg(test)]
// Tests should fail loudly on a broken fixture; unwrapping is the clearest way.
#[allow(clippy::unwrap_used)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;
    use crate::types::{RunState, Trigger};

    fn run(id: &str, agent: &str, parent: Option<&str>, minute: u32, state: RunState) -> Run {
        Run {
            id: id.to_owned(),
            project: "p".to_owned(),
            agent: agent.to_owned(),
            state,
            trigger: Trigger::Manual,
            parent_run_id: parent.map(ToOwned::to_owned),
            params: None,
            tmux_session: None,
            log_path: None,
            workdir: None,
            model: None,
            created_at: Utc.with_ymd_and_hms(2026, 1, 1, 0, minute, 0).unwrap(),
            started_at: None,
            finished_at: None,
            exit_code: None,
            cost_usd: None,
            num_turns: None,
            usage_json: None,
        }
    }

    #[test]
    fn flow_tree_unions_every_branch_the_job_has_taken() {
        // Chain 1: root -> ctx -> impl -> report. Chain 2: root -> ctx ->
        // investigate -> report. The tree must show BOTH branches, with
        // report under each — the latest run alone would hide one.
        let runs = vec![
            run("a1", "root", None, 0, RunState::Succeeded),
            run("a2", "ctx", Some("a1"), 1, RunState::Succeeded),
            run("a3", "impl", Some("a2"), 2, RunState::Succeeded),
            run("a4", "report", Some("a3"), 3, RunState::Succeeded),
            run("b1", "root", None, 4, RunState::Succeeded),
            run("b2", "ctx", Some("b1"), 5, RunState::Succeeded),
            run("b3", "investigate", Some("b2"), 6, RunState::Succeeded),
            run("b4", "report", Some("b3"), 7, RunState::Succeeded),
        ];
        let tree = flow_tree("p", "root", &runs);
        let flat: Vec<(&str, usize)> = tree.iter().map(|(a, d)| (a.as_str(), *d)).collect();
        assert_eq!(
            flat,
            vec![
                ("root", 0),
                ("ctx", 1),
                ("impl", 2),
                ("report", 3),
                ("investigate", 2),
                ("report", 3),
            ]
        );
    }

    #[test]
    fn flow_tree_of_a_job_that_never_ran_is_just_the_job() {
        assert_eq!(
            flow_tree("p", "lonely", &[]),
            vec![("lonely".to_owned(), 0)]
        );
    }

    #[test]
    fn flow_tree_ignores_other_jobs_chains() {
        let runs = vec![
            run("a1", "root", None, 0, RunState::Succeeded),
            run("a2", "mine", Some("a1"), 1, RunState::Succeeded),
            run("b1", "other", None, 2, RunState::Succeeded),
            run("b2", "theirs", Some("b1"), 3, RunState::Succeeded),
        ];
        let names: Vec<String> = flow_tree("p", "root", &runs)
            .into_iter()
            .map(|(a, _)| a)
            .collect();
        assert_eq!(names, vec!["root".to_owned(), "mine".to_owned()]);
    }
}
