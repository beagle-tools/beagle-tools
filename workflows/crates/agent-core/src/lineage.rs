//! The observed run lineage: who triggered whom, and what a chain cost.
//!
//! This lives in `agent-core` rather than in a UI crate because both
//! agenttop and Agentdesk draw it, and a DAG that disagrees between the
//! two views would be worse than no DAG at all. The rules — what counts
//! as a root, how children order, when a cost is a floor rather than a
//! total — are domain decisions, not drawing decisions.

use std::collections::HashMap;

use crate::types::{Run, RunState};

/// Cost of a run and everything it triggered.
///
/// `partial` marks the figure as a FLOOR, not a bill. A run that has not
/// reported a cost is either still going or run by a runner that reports
/// no price (codex), and quietly summing those as zero would understate a
/// chain by whatever the unfinished half of it costs.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SubtreeCost {
    /// Sum of every reported `cost_usd` in the subtree, in US dollars.
    pub total: f64,
    /// True when some run in the subtree has no reported cost yet.
    pub partial: bool,
}

/// An indexed view of a set of runs by parent/child.
#[derive(Debug, Default)]
pub struct Lineage<'a> {
    children: HashMap<&'a str, Vec<&'a Run>>,
    roots: Vec<&'a Run>,
}

impl<'a> Lineage<'a> {
    /// Index the runs. A run whose parent is not in this set is treated as
    /// a root: the parent has usually been pruned by retention, and
    /// hiding the child because its parent expired would make a live
    /// chain vanish from the view.
    pub fn build<I>(runs: I) -> Self
    where
        I: IntoIterator<Item = &'a Run>,
    {
        let runs: Vec<&Run> = runs.into_iter().collect();
        let by_id: HashMap<&str, &Run> = runs.iter().map(|r| (r.id.as_str(), *r)).collect();
        let mut children: HashMap<&str, Vec<&Run>> = HashMap::new();
        let mut roots: Vec<&Run> = Vec::new();
        for run in &runs {
            match run.parent_run_id.as_deref().and_then(|p| by_id.get(p)) {
                Some(parent) => children.entry(parent.id.as_str()).or_default().push(run),
                None => roots.push(run),
            }
        }
        // Newest chain first, but within a chain oldest first — a chain
        // reads in the order it happened.
        roots.sort_by_key(|r| std::cmp::Reverse(r.created_at));
        for kids in children.values_mut() {
            kids.sort_by_key(|r| r.created_at);
        }
        Self { children, roots }
    }

    /// Runs with no parent in this set (their parent may simply be outside
    /// it), newest first.
    pub fn roots(&self) -> &[&'a Run] {
        &self.roots
    }

    /// Direct children of run `id`, oldest first; empty for unknown ids.
    pub fn children_of(&self, id: &str) -> &[&'a Run] {
        self.children.get(id).map_or(&[], Vec::as_slice)
    }

    /// Every run in display order with its depth, optionally narrowed to
    /// one root.
    ///
    /// One function so a renderer and its selection logic can never
    /// disagree about which row is which.
    pub fn rows(&self, focus: Option<&str>) -> Vec<(&'a Run, usize)> {
        let mut out = Vec::new();
        for root in &self.roots {
            if focus.is_some_and(|f| f != root.id) {
                continue;
            }
            self.walk(&mut out, root, 0);
        }
        out
    }

    fn walk(&self, out: &mut Vec<(&'a Run, usize)>, run: &'a Run, depth: usize) {
        out.push((run, depth));
        for child in self.children_of(&run.id) {
            self.walk(out, child, depth + 1);
        }
    }

    /// Cost of `run` plus everything it triggered, transitively.
    pub fn subtree_cost(&self, run: &Run) -> SubtreeCost {
        let mut total = run.cost_usd.unwrap_or(0.0);
        let mut partial = run.cost_usd.is_none() && run.state != RunState::Succeeded;
        for child in self.children_of(&run.id) {
            let nested = self.subtree_cost(child);
            total += nested.total;
            partial |= nested.partial;
        }
        SubtreeCost { total, partial }
    }
}

#[cfg(test)]
// Tests should fail loudly on a broken fixture; unwrapping is the clearest way.
#[allow(clippy::unwrap_used)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;
    use crate::types::Trigger;

    fn run(id: &str, parent: Option<&str>, minute: u32, cost: Option<f64>, state: RunState) -> Run {
        Run {
            id: id.to_owned(),
            project: "p".to_owned(),
            agent: "a".to_owned(),
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
            cost_usd: cost,
            num_turns: None,
            usage_json: None,
        }
    }

    #[test]
    fn children_nest_under_their_parent_oldest_first() {
        let runs = vec![
            run("root", None, 0, None, RunState::Succeeded),
            run("b", Some("root"), 2, None, RunState::Succeeded),
            run("a", Some("root"), 1, None, RunState::Succeeded),
        ];
        let lineage = Lineage::build(&runs);
        let rows: Vec<(&str, usize)> = lineage
            .rows(None)
            .iter()
            .map(|(r, d)| (r.id.as_str(), *d))
            .collect();
        assert_eq!(rows, vec![("root", 0), ("a", 1), ("b", 1)]);
    }

    #[test]
    fn a_run_whose_parent_was_pruned_is_still_shown_as_a_root() {
        // Retention deletes the parent long before the child; the child
        // must not disappear with it.
        let runs = vec![run(
            "orphan",
            Some("swept-away"),
            0,
            None,
            RunState::Running,
        )];
        let lineage = Lineage::build(&runs);
        assert_eq!(lineage.roots().len(), 1);
        assert_eq!(lineage.rows(None).len(), 1);
    }

    #[test]
    fn subtree_cost_sums_both_depth_and_fan_out() {
        // root -> a -> b, plus root -> c: the shape the linear pipeline
        // makes. A sum that only walked the first child would pass a
        // straight-chain test and still be wrong here.
        let runs = vec![
            run("root", None, 0, Some(0.01), RunState::Succeeded),
            run("a", Some("root"), 1, Some(1.00), RunState::Succeeded),
            run("b", Some("a"), 2, Some(4.00), RunState::Succeeded),
            run("c", Some("root"), 3, Some(0.50), RunState::Succeeded),
        ];
        let lineage = Lineage::build(&runs);
        let cost = lineage.subtree_cost(&runs[0]);
        assert!((cost.total - 5.51).abs() < 1e-9, "got {}", cost.total);
        assert!(!cost.partial, "every run reported, so the total is final");
    }

    #[test]
    fn an_unfinished_child_makes_the_total_a_floor() {
        let runs = vec![
            run("root", None, 0, Some(1.0), RunState::Succeeded),
            run("kid", Some("root"), 1, None, RunState::Running),
        ];
        let lineage = Lineage::build(&runs);
        let cost = lineage.subtree_cost(&runs[0]);
        assert!((cost.total - 1.0).abs() < f64::EPSILON);
        assert!(cost.partial, "a running child means the bill is not final");
    }

    #[test]
    fn a_succeeded_run_that_reported_no_cost_is_not_partial() {
        // codex reports no price. That is a complete answer of "nothing
        // known", not a pending one — flagging it would mark every codex
        // chain as an underestimate forever.
        let runs = vec![run("root", None, 0, None, RunState::Succeeded)];
        let lineage = Lineage::build(&runs);
        assert!(!lineage.subtree_cost(&runs[0]).partial);
    }

    #[test]
    fn focus_narrows_to_one_chain() {
        let runs = vec![
            run("one", None, 0, None, RunState::Succeeded),
            run("two", None, 1, None, RunState::Succeeded),
            run("two-kid", Some("two"), 2, None, RunState::Succeeded),
        ];
        let lineage = Lineage::build(&runs);
        let rows = lineage.rows(Some("two"));
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].0.id, "two");
    }
}
