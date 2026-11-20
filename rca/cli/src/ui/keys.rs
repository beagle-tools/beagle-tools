//! Keypress → state transition. Every branch here is unit-testable without
//! a terminal: `handle_key` mutates [`App`] and returns whether to keep
//! running. The top-level dispatch lives here; `modal` (agents screen,
//! overlays, help), `filter` (filter mode), and `navigation` (list/content
//! movement) hold the per-mode handlers.

mod filter;
mod modal;
mod navigation;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{App, Focus, Screen, Tab};

/// What the key loop should do next. `Edit` carries the file to open —
/// state transitions stay pure; the event loop owns the terminal and does
/// the suspend/spawn/restore dance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Flow {
    Continue,
    Quit,
    /// Suspend the TUI and open this file in the user's editor.
    Edit(std::path::PathBuf),
    /// Suspend, open the agents config in the editor, then ask the daemon to
    /// reload it.
    EditAgentsConfig(std::path::PathBuf),
}

impl App {
    pub(crate) fn handle_key(&mut self, key: KeyEvent) -> Flow {
        self.status = None;
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl && key.code == KeyCode::Char('c') {
            return Flow::Quit;
        }
        if self.side.is_active() {
            let total = self.side.last_total();
            self.status = self.side.handle_key(key, total).status;
            return Flow::Continue;
        }
        if self.route_modal_key(key) {
            return Flow::Continue;
        }
        // The agents screen is a separate top-level view; while it owns the
        // screen, only a small set of keys apply.
        if self.screen() == Screen::Agents {
            return self.handle_agents_key(key);
        }
        match key.code {
            // `A` switches to the agents screen; see `handle_agents_key` for the
            // way back.
            KeyCode::Char('A') => self.toggle_screen(),
            // `q` and `Q` quit, as in every beagle TUI (`docs/CODING_STANDARDS.md`
            // "Terminal UIs"). Ctrl-C is handled above.
            KeyCode::Char('q' | 'Q') => return Flow::Quit,
            KeyCode::Char('?') => {
                self.show_help = true;
                self.help_filter = None;
            }
            KeyCode::Char('T') => self.open_toolbox(),
            KeyCode::Char('o') => self.open_links(),
            KeyCode::Char('R') => self.open_related(),
            KeyCode::Char('V') => self.verify_final_review(),
            KeyCode::Char('t') => self.open_status_picker(),
            KeyCode::Char('P') => self.open_attach_prompt(super::overlays::AttachKind::Pr),
            KeyCode::Char('I') => self.open_attach_prompt(super::overlays::AttachKind::Ticket),
            KeyCode::Char('#') => self.open_tags_editor(),
            KeyCode::Char('!') => self.open_errors(),
            KeyCode::Char('S') => self.open_settings(),
            // `/` always searches keywords across the selected incident;
            // `f` filters the incident list; `F` follows (tail -f). Each
            // also moves the focus to where the action is.
            KeyCode::Char('/') => self.start_content_search(),
            KeyCode::Char('\\') => self.open_finder(),
            KeyCode::Char('f') => {
                self.filter_input = super::FilterInput::Facets;
                self.focus = Focus::List;
            }
            // Shift-D, list focus only: deleting is a list-management
            // action, and the guard keeps a stray D while reading content
            // from opening a destructive prompt.
            KeyCode::Char('D') if self.focus == Focus::List => self.open_confirm_delete(),
            KeyCode::Char('b') => self.focus = Focus::List,
            KeyCode::Char('s') => self.toggle_sidebar(),
            KeyCode::Char('w') => self.start_resize(),
            KeyCode::Char('a') => self.toggle_archived(),
            KeyCode::Char('E') => {
                if let Some(flow) = self.edit_request() {
                    return flow;
                }
            }
            KeyCode::Char('c') => self.copy_current_tab(),
            KeyCode::Char('C') => self.copy_workspace(),
            // `y` yanks the content line (or visual selection); `i` copies
            // the incident id from anywhere.
            KeyCode::Char('y') => {
                if self.focus == Focus::Content {
                    self.yank_content();
                } else {
                    self.status =
                        Some("open an incident (enter) to yank lines — i copies the id".to_owned());
                }
            }
            KeyCode::Char('i') => self.copy_id(),
            KeyCode::Char('x') => self.export_current(),
            KeyCode::Char('d') if ctrl && self.focus == Focus::Content => self.half_page(true),
            KeyCode::Char('u') if ctrl && self.focus == Focus::Content => self.half_page(false),
            KeyCode::Char('r') => {
                let _ = self.reload();
                self.status = Some("reloaded".to_owned());
            }
            KeyCode::Char('F') => self.toggle_follow(),
            KeyCode::Tab | KeyCode::Char(']') | KeyCode::Right => {
                self.switch_tab(self.next_tab());
            }
            KeyCode::BackTab | KeyCode::Char('[') | KeyCode::Left => {
                self.switch_tab(self.prev_tab());
            }
            KeyCode::Char(c @ '1'..='9') => {
                // '1'..='9' index into the *visible* tabs for this incident,
                // so hidden optional tabs don't leave gaps in the numbering.
                let index = (c as usize).saturating_sub('1' as usize);
                if let Some(tab) = self.visible_tabs().get(index) {
                    self.switch_tab(*tab);
                }
            }
            // A committed search owns n/N until esc clears it — including on
            // the Diagrams tab, where p still cycles diagrams.
            KeyCode::Char('n') if self.content_search.is_some() => self.search_step(1),
            KeyCode::Char('N') if self.content_search.is_some() => self.search_step(-1),
            KeyCode::Char('n') if self.tab == Tab::Diagrams => self.cycle_diagram(1),
            KeyCode::Char('p') if self.tab == Tab::Diagrams => self.cycle_diagram(-1),
            _ => match self.focus {
                Focus::List => self.handle_list_key(key.code),
                Focus::Content => self.handle_content_key(key.code),
            },
        }
        // Invariant: the sidebar is never collapsed while the list has
        // focus — `b`, esc, `f`, and every other back-to-list path would
        // otherwise leave the selection cursor invisible. Leaving the
        // content also ends any visual-line selection.
        if self.focus == Focus::List {
            self.sidebar_collapsed = false;
            self.clear_visual_select();
        }
        Flow::Continue
    }

    /// `E`: the file behind the current tab, to open in the user's editor.
    /// Section tabs edit their markdown file (created by the editor if the
    /// section doesn't exist yet); the Diagrams tab edits the current
    /// diagram. `None` (with a status message) when there is nothing to
    /// edit.
    fn edit_request(&mut self) -> Option<Flow> {
        let Some(rca) = self.selected_rca() else {
            self.status = Some("no incident selected — nothing to edit".to_owned());
            return None;
        };
        let id = rca.id.clone();
        if let Some(kind) = self.tab.section() {
            let path = self.store.workspace_dir(&id).join(kind.file_name());
            return Some(Flow::Edit(path));
        }
        // Diagrams tab: edit the diagram currently on screen.
        match self.store.list_diagrams(&id) {
            Ok(diagrams) if !diagrams.is_empty() => {
                let index = self.diagram_index.min(diagrams.len() - 1);
                Some(Flow::Edit(diagrams[index].path.clone()))
            }
            _ => {
                self.status = Some("no diagram to edit — add one under diagrams/ first".to_owned());
                None
            }
        }
    }

    /// `F`: follow (tail -f) the content, or stop following.
    fn toggle_follow(&mut self) {
        self.follow = !self.follow;
        if self.follow {
            self.scroll = u16::MAX; // jump to the tail immediately
            self.status = Some("following — reloads stick to the bottom · esc exits".to_owned());
        } else {
            self.status = Some("follow off".to_owned());
        }
    }

    /// `a`: toggle archived incidents into or out of the list, keeping the
    /// current selection when it survives the toggle.
    fn toggle_archived(&mut self) {
        self.show_archived = !self.show_archived;
        let keep = self.selected_rca().map(|rca| rca.id.clone());
        self.recompute_visible(keep);
        let archived = self.archived_count();
        self.status = Some(if self.show_archived {
            format!("showing {archived} archived (dimmed) — a hides them")
        } else {
            "archived hidden — a shows them".to_owned()
        });
    }

    /// `s`: collapse the sidebar for a full-width content pane, or bring
    /// it back. Collapsing moves focus to the content — a hidden list
    /// cannot hold the cursor.
    fn toggle_sidebar(&mut self) {
        self.sidebar_collapsed = !self.sidebar_collapsed;
        if self.sidebar_collapsed {
            self.focus = Focus::Content;
            self.status = Some("sidebar hidden — s or b brings it back".to_owned());
        } else {
            self.status = Some("sidebar shown".to_owned());
        }
    }

    /// `w`: resize the sidebar, unless it is hidden.
    fn start_resize(&mut self) {
        if self.sidebar_collapsed {
            self.status = Some("sidebar hidden — s shows it, then w resizes it".to_owned());
        } else {
            self.side.start();
        }
    }

    pub(super) fn select(&mut self, index: usize) {
        if self.visible.is_empty() {
            self.selected = 0;
            return;
        }
        let clamped = index.min(self.visible.len() - 1);
        if clamped != self.selected {
            self.selected = clamped;
            self.diagram_index = 0;
            self.reset_scroll();
        }
    }

    pub(super) fn switch_tab(&mut self, tab: Tab) {
        // Tabs belong to a selected incident. With nothing visible the
        // welcome screen has no tab bar, so a silent state change would
        // read as a broken keybinding — say why instead.
        if self.visible.is_empty() {
            self.status = Some(if self.rcas.is_empty() {
                "no incidents yet — tabs appear once a workspace exists \
                 (beagle rca new <slug> --title \"...\")"
                    .to_owned()
            } else {
                "no incident matches the filter — esc clears it".to_owned()
            });
            return;
        }
        if tab != self.tab {
            self.tab = tab;
            self.reset_scroll();
        }
        self.focus = Focus::Content;
    }

    /// Clamps vertical scroll to the real content height so the view can
    /// never scroll into the void.
    pub(super) fn scroll_to(&mut self, target: u16) {
        let max = self
            .viewport
            .content_lines
            .saturating_sub(self.viewport.height);
        self.scroll = target.min(max);
    }

    pub(crate) fn reset_scroll(&mut self) {
        self.scroll = 0;
        self.hscroll = 0;
        self.content_cursor = 0;
        self.content_select_anchor = None;
    }
}

#[cfg(test)]
#[path = "tests/keys.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/keys_filter.rs"]
mod tests_filter;

#[cfg(test)]
#[path = "tests/keys_editors.rs"]
mod tests_editors;

#[cfg(test)]
#[path = "tests/keys_actions.rs"]
mod tests_actions;

#[cfg(test)]
#[path = "tests/keys_standard.rs"]
mod tests_standard;
