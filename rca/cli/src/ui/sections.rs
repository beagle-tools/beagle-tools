//! Per-incident section state: which tabs exist on disk, tab cycling over
//! them, unread/checklist tracking, and polled PR states.

use crate::model::{RcaId, SectionKind};

use super::{App, Tab};

impl App {
    /// The newest section-file modification across workspace `id`, straight
    /// from the mtime snapshot kept for unread tracking — zero extra I/O.
    pub(crate) fn last_activity(&self, id: &RcaId) -> Option<std::time::SystemTime> {
        self.mtimes
            .iter()
            .filter(|((mtime_id, _), _)| mtime_id == id)
            .map(|(_, mtime)| *mtime)
            .max()
    }

    /// Whether the selected workspace has `section` on disk. Checked
    /// against the file directly (a stat) so a created or deleted optional
    /// section flips the tab's visibility immediately, without waiting for
    /// the mtime snapshot — and without a stale entry keeping it "present".
    pub(super) fn has_section(&self, kind: SectionKind) -> bool {
        self.selected_rca().is_some_and(|rca| {
            self.store
                .workspace_dir(&rca.id)
                .join(kind.file_name())
                .exists()
        })
    }

    /// The tabs to show for the selected incident: every core tab, plus
    /// each optional tab whose backing file exists. Navigation and the tab
    /// bar operate on this set so hidden tabs are never reachable.
    pub(crate) fn visible_tabs(&self) -> Vec<Tab> {
        Tab::ALL
            .iter()
            .copied()
            .filter(|tab| {
                !tab.is_optional() || tab.section().is_some_and(|kind| self.has_section(kind))
            })
            .collect()
    }

    /// The next visible tab after the current one, wrapping.
    pub(crate) fn next_tab(&self) -> Tab {
        self.step_tab(true)
    }

    /// The previous visible tab, wrapping.
    pub(crate) fn prev_tab(&self) -> Tab {
        self.step_tab(false)
    }

    pub(super) fn step_tab(&self, forward: bool) -> Tab {
        let tabs = self.visible_tabs();
        if tabs.is_empty() {
            return self.tab;
        }
        let here = tabs.iter().position(|t| *t == self.tab).unwrap_or(0);
        let len = tabs.len();
        let idx = if forward {
            (here + 1) % len
        } else {
            (here + len - 1) % len
        };
        tabs[idx]
    }

    /// If the current tab is not among the visible ones (e.g. the incident
    /// changed and lost its `commands.md`), fall back to Summary.
    pub(crate) fn ensure_tab_visible(&mut self) {
        if !self.visible_tabs().contains(&self.tab) {
            self.tab = Tab::Summary;
        }
    }

    /// Whether `tab` of workspace `id` changed on disk since last viewed.
    pub(crate) fn is_unread(&self, id: &RcaId, tab: Tab) -> bool {
        tab.section()
            .is_some_and(|kind| self.unread.contains(&(id.clone(), kind)))
    }

    /// Whether any section of workspace `id` is unread.
    pub(crate) fn has_unread(&self, id: &RcaId) -> bool {
        self.unread.iter().any(|(unread_id, _)| unread_id == id)
    }

    /// Aggregate checkbox progress across every section of workspace `id`:
    /// `(checked, total)`, or `None` when no section has a checklist.
    pub(crate) fn checklist_progress(&self, id: &RcaId) -> Option<(usize, usize)> {
        let (checked, total) = self
            .checklists
            .iter()
            .filter(|((list_id, _), _)| list_id == id)
            .fold((0, 0), |(c, t), (_, &(checked, total))| {
                (c + checked, t + total)
            });
        (total > 0).then_some((checked, total))
    }

    /// Every unique attached-PR URL across all workspaces, sorted, for the
    /// status poller.
    pub(super) fn pr_urls(&self) -> Vec<String> {
        let mut urls: Vec<String> = Vec::new();
        for rca in &self.rcas {
            for url in &rca.meta.prs {
                if !urls.contains(url) {
                    urls.push(url.clone());
                }
            }
        }
        urls.sort();
        urls
    }

    /// The polled state of one attached PR, if known.
    pub(crate) fn pr_state(&self, url: &str) -> Option<crate::prs::PrState> {
        self.pr_states.get(url).copied()
    }
}
