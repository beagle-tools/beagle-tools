//! The modal overlays: the `o` link popup, the `R` related-incidents popup,
//! the `T` toolbox (`toolbox`), and the editing popups (`editors`). Each
//! owns the keys while open, so `q`/`j`/`k` scroll or close instead of
//! leaking into the app underneath.

mod editors;
mod toolbox;

use crossterm::event::KeyCode;

use crate::model::{RcaId, Severity, Status};

use super::App;

/// State of the `o` link popup.
#[derive(Debug)]
pub(crate) struct LinksPopup {
    /// Links to offer: attached PRs first, then URLs from the current tab.
    pub items: Vec<String>,
    /// Index of the highlighted link.
    pub selected: usize,
}

/// One row of the `R` related-incidents popup.
#[derive(Debug, Clone)]
pub(crate) struct RelatedItem {
    /// The related workspace.
    pub id: RcaId,
    /// Its severity (for the badge).
    pub severity: Severity,
    /// Its status (for the glyph).
    pub status: Status,
    /// Its title.
    pub title: String,
    /// Why it ranked: `system: alloy` / `2 systems, tag: ingestion`.
    pub shared: String,
}

/// What an [`AttachPrompt`] attaches on enter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AttachKind {
    /// A remediation PR (`P`).
    Pr,
    /// A tracking ticket (`I`).
    Ticket,
}

impl AttachKind {
    /// Prompt title.
    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::Pr => " attach a PR ",
            Self::Ticket => " attach a ticket ",
        }
    }

    /// Input-field label.
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Pr => "  PR URL: ",
            Self::Ticket => "  ticket: ",
        }
    }
}

/// State of the `P`/`I` attach prompt: the workspace the reference will be
/// attached to, what kind it is, and the text being typed. Pinned by id at
/// open time.
#[derive(Debug)]
pub(crate) struct AttachPrompt {
    /// What enter attaches.
    pub kind: AttachKind,
    /// The workspace the reference attaches to.
    pub id: RcaId,
    /// Its title, for the prompt header.
    pub title: String,
    /// The URL/reference being typed.
    pub input: String,
}

/// State of the `D` delete confirmation popup: the workspace it will
/// delete if confirmed. Pinned by id at open time, so a selection change
/// underneath (e.g. a reload) can never redirect the delete.
#[derive(Debug)]
pub(crate) struct ConfirmDelete {
    /// The workspace to delete on `y`.
    pub id: RcaId,
    /// Its title, shown so the user confirms the right incident.
    pub title: String,
}

/// State of the `t` status picker: the workspace whose status it will
/// change on enter. Pinned by id at open time, like [`ConfirmDelete`].
#[derive(Debug)]
pub(crate) struct StatusPicker {
    /// The workspace whose status changes on enter.
    pub id: RcaId,
    /// Its title, for the popup header.
    pub title: String,
    /// Its status when the picker opened — marked, and a no-op if re-picked.
    pub current: Status,
    /// Index into [`Status::ALL`] of the highlighted stage.
    pub selected: usize,
}

/// State of the `#` tags editor: the workspace whose tags it edits,
/// pinned by id at open time like the other mutating popups.
#[derive(Debug)]
pub(crate) struct TagsEditor {
    /// The workspace whose tags change on every edit.
    pub id: RcaId,
    /// Its title, for the popup header.
    pub title: String,
    /// The tags as currently on disk.
    pub tags: Vec<String>,
    /// Highlighted row: an index into `tags`, or `tags.len()` for the
    /// trailing `+ add tag` row.
    pub selected: usize,
    /// `Some` while typing a new tag; the buffer is the partial text.
    pub typing: Option<String>,
}

/// State of the `R` related-incidents popup.
#[derive(Debug)]
pub(crate) struct RelatedPopup {
    /// Related workspaces, best match first.
    pub items: Vec<RelatedItem>,
    /// Index of the highlighted row.
    pub selected: usize,
}

impl App {
    /// Builds and opens the `o` popup: the workspace's attached PRs first,
    /// then every URL found in the current tab's raw content.
    pub(crate) fn open_links(&mut self) {
        let Some(rca) = self.selected_rca() else {
            self.status = Some("no workspace selected".to_owned());
            return;
        };
        let id = rca.id.clone();
        let mut items: Vec<String> = rca.meta.prs.clone();
        let raw = match self.tab.section() {
            Some(kind) => self.store.read_section(&id, kind).ok().flatten(),
            None => self
                .current_diagram_raw(&id)
                .ok()
                .flatten()
                .map(|(_, content)| content),
        };
        if let Some(raw) = raw {
            for url in crate::links::extract_urls(&raw) {
                if !items.contains(&url) {
                    items.push(url);
                }
            }
        }
        if items.is_empty() {
            self.status = Some("no attached PRs or links on this tab".to_owned());
            return;
        }
        self.links = Some(LinksPopup { items, selected: 0 });
    }

    /// Keystrokes while the link popup is open: pick, open, or close.
    pub(crate) fn handle_links_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc | KeyCode::Char('q' | 'o') => self.links = None,
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(popup) = self.links.as_mut() {
                    popup.selected = (popup.selected + 1).min(popup.items.len().saturating_sub(1));
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(popup) = self.links.as_mut() {
                    popup.selected = popup.selected.saturating_sub(1);
                }
            }
            KeyCode::Enter => {
                let url = self
                    .links
                    .take()
                    .and_then(|popup| popup.items.get(popup.selected).cloned());
                if let Some(url) = url {
                    self.status = Some(match crate::links::open_url(&url) {
                        Ok(()) => format!("opened {url}"),
                        Err(e) => format!("open failed: {e}"),
                    });
                }
            }
            _ => {}
        }
    }

    /// The link popup, when open.
    pub(crate) fn links(&self) -> Option<&LinksPopup> {
        self.links.as_ref()
    }

    /// Builds and opens the `R` popup: past workspaces sharing systems or
    /// tags with the selected one, best match first.
    pub(crate) fn open_related(&mut self) {
        let Some(rca) = self.selected_rca() else {
            self.status = Some("no workspace selected".to_owned());
            return;
        };
        let items: Vec<RelatedItem> = crate::similar::rank(rca, &self.rcas)
            .iter()
            .map(|entry| RelatedItem {
                id: entry.rca.id.clone(),
                severity: entry.rca.meta.severity,
                status: entry.rca.meta.status,
                title: entry.rca.meta.title.clone(),
                shared: crate::similar::shared_label(entry),
            })
            .collect();
        if items.is_empty() {
            self.status = Some(
                "no related incidents — nothing shares systems or tags with this one".to_owned(),
            );
            return;
        }
        self.related = Some(RelatedPopup { items, selected: 0 });
    }

    /// Keystrokes while the related popup is open: pick, jump, or close.
    pub(crate) fn handle_related_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc | KeyCode::Char('q' | 'R') => self.related = None,
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(popup) = self.related.as_mut() {
                    popup.selected = (popup.selected + 1).min(popup.items.len().saturating_sub(1));
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(popup) = self.related.as_mut() {
                    popup.selected = popup.selected.saturating_sub(1);
                }
            }
            KeyCode::Enter => {
                let id = self
                    .related
                    .take()
                    .and_then(|popup| popup.items.get(popup.selected).map(|item| item.id.clone()));
                if let Some(id) = id {
                    self.jump_to_workspace(&id);
                }
            }
            _ => {}
        }
    }

    /// Selects workspace `id` in the sidebar, clearing any fuzzy filter
    /// that would hide it.
    fn jump_to_workspace(&mut self, id: &RcaId) {
        self.filter.clear();
        self.filter_input = crate::ui::FilterInput::Off;
        self.recompute_visible(Some(id.clone()));
        self.diagram_index = 0;
        self.reset_scroll();
        self.status = Some(format!("jumped to {id}"));
    }

    /// The related popup, when open.
    pub(crate) fn related(&self) -> Option<&RelatedPopup> {
        self.related.as_ref()
    }
}

#[cfg(test)]
#[path = "tests/overlays.rs"]
mod tests;
