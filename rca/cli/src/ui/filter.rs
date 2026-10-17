//! The sidebar filter: facet + fuzzy narrowing of the incident list and the
//! accessors the sidebar and filter prompt render from.

use crate::model::RcaId;
use crate::model::RcaSummary;

use super::{App, FilterInput};

impl App {
    /// Re-runs the filter over the workspace list, keeping the selection
    /// pinned to `keep` when that workspace still matches. Facets narrow
    /// the candidate set (a workspace must match every non-empty facet
    /// dimension), then the fuzzy query ranks within it.
    pub(super) fn recompute_visible(&mut self, keep: Option<RcaId>) {
        let mut scored: Vec<(i32, usize)> = self
            .rcas
            .iter()
            .enumerate()
            .filter(|(_, rca)| self.show_archived || !rca.archived)
            .filter(|(_, rca)| {
                self.facet_statuses.is_empty() || self.facet_statuses.contains(&rca.meta.status)
            })
            .filter(|(_, rca)| {
                self.facet_severities.is_empty()
                    || self.facet_severities.contains(&rca.meta.severity)
            })
            .filter(|(_, rca)| !self.facet_has_pr || !rca.meta.prs.is_empty())
            .filter(|(_, rca)| {
                !self.facet_security
                    || rca
                        .meta
                        .tags
                        .iter()
                        .any(|t| t == crate::model::SECURITY_TAG)
            })
            .filter_map(|(index, rca)| {
                let haystack = format!(
                    "{} {} {} {}",
                    rca.meta.title,
                    rca.id,
                    rca.meta.systems.join(" "),
                    rca.meta.tags.join(" "),
                );
                crate::fuzzy::score(&self.filter, &haystack).map(|s| (s, index))
            })
            .collect();
        // Best match first; ties keep the sidebar's severity/recency order.
        scored.sort_by_key(|&(score, index)| (-score, index));
        self.visible = scored.into_iter().map(|(_, index)| index).collect();
        self.selected = keep
            .and_then(|id| self.visible.iter().position(|&i| self.rcas[i].id == id))
            .unwrap_or(0);
    }

    /// Whether anything (facets or free text) is narrowing the list.
    pub(crate) fn has_active_filter(&self) -> bool {
        !self.filter.is_empty()
            || !self.facet_statuses.is_empty()
            || !self.facet_severities.is_empty()
            || self.facet_has_pr
            || self.facet_security
    }

    /// Clears every filter dimension and restores the full list, keeping
    /// the current selection when it survives.
    pub(crate) fn clear_filter(&mut self) {
        let keep = self.selected_rca().map(|r| r.id.clone());
        self.filter.clear();
        self.facet_statuses.clear();
        self.facet_severities.clear();
        self.facet_has_pr = false;
        self.facet_security = false;
        self.recompute_visible(keep);
    }

    /// The active facets as a compact label for the sidebar title and the
    /// filter prompt: `[high · investigating]`. Empty when no facets are on.
    pub(crate) fn facet_label(&self) -> String {
        let mut parts: Vec<&str> = Vec::new();
        for severity in crate::model::Severity::ALL {
            if self.facet_severities.contains(&severity) {
                parts.push(severity.as_str());
            }
        }
        for status in crate::model::Status::ALL {
            if self.facet_statuses.contains(&status) {
                parts.push(status.as_str());
            }
        }
        if self.facet_has_pr {
            parts.push("has PR");
        }
        if self.facet_security {
            parts.push("security");
        }
        if parts.is_empty() {
            String::new()
        } else {
            format!("[{}]", parts.join(" · "))
        }
    }

    pub(crate) fn filter_typing(&self) -> bool {
        self.filter_input == FilterInput::Typing
    }

    /// Workspaces matching the current filter, best match first.
    pub(crate) fn visible_rcas(&self) -> impl Iterator<Item = &RcaSummary> {
        self.visible
            .iter()
            .filter_map(|&index| self.rcas.get(index))
    }

    pub(crate) fn visible_len(&self) -> usize {
        self.visible.len()
    }

    pub(crate) fn filter(&self) -> &str {
        &self.filter
    }

    pub(crate) fn search_active(&self) -> bool {
        self.filter_input != FilterInput::Off
    }
}
