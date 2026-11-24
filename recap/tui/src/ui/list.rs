//! The PR list's grouping: a heading per state, then its PRs.

use crate::store::RecapSummary;

/// One line-group of the PR list: a state heading, or a PR (by index into
/// the rows).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ListEntry {
    /// `── DRAFTS · 2 ──`: the state and how many PRs are under it.
    Header(crate::model::PrState, usize),
    /// A PR row.
    Row(usize),
}

/// The PR list partitioned by state: a heading before each non-empty group,
/// in `PrState` order (drafts, open, merged, closed). `rows` arrive sorted
/// that way from the store; the grouping doesn't depend on it, though —
/// each state's rows are collected explicitly, keeping their order.
pub(crate) fn list_entries(rows: &[RecapSummary]) -> Vec<ListEntry> {
    let mut out = Vec::with_capacity(rows.len() + 4);
    for state in crate::model::PrState::ALL {
        let members: Vec<usize> = (0..rows.len())
            .filter(|&i| rows[i].state == state)
            .collect();
        if members.is_empty() {
            continue;
        }
        out.push(ListEntry::Header(state, members.len()));
        out.extend(members.into_iter().map(ListEntry::Row));
    }
    out
}
