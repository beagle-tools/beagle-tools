//! Navigation keys for the focused pane: list selection, content scrolling
//! and the line cursor, and diagram cycling.

use crossterm::event::KeyCode;

use crate::ui::{App, Focus, Pane};

impl App {
    pub(super) fn handle_list_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('j') | KeyCode::Down => self.select(self.selected.saturating_add(1)),
            KeyCode::Char('k') | KeyCode::Up => self.select(self.selected.saturating_sub(1)),
            KeyCode::Char('g') | KeyCode::Home => self.select(0),
            KeyCode::Char('G') | KeyCode::End => self.select(usize::MAX),
            KeyCode::Esc if self.has_active_filter() => self.clear_filter(),
            KeyCode::Esc if self.follow => {
                self.follow = false;
                self.status = Some("follow off".to_owned());
            }
            KeyCode::Enter | KeyCode::Char('l') if !self.visible.is_empty() => {
                // Opening an incident consumes the filter (text and facets
                // alike): the pick has been made, so bring the full list
                // back with the chosen incident still selected.
                if self.has_active_filter() {
                    self.clear_filter();
                }
                self.focus = Focus::Content;
            }
            _ => {}
        }
    }

    pub(super) fn handle_content_key(&mut self, code: KeyCode) {
        let page = isize::try_from(self.viewport.height.saturating_sub(1).max(1)).unwrap_or(1);
        match code {
            // Esc peels one layer: visual selection, then search, follow,
            // then back to the list.
            KeyCode::Esc if self.selection_lines().is_some() => {
                self.clear_visual_select();
                self.status = Some("selection cancelled".to_owned());
            }
            KeyCode::Esc if self.content_search.is_some() => self.clear_content_search(),
            KeyCode::Esc if self.follow => {
                self.follow = false;
                self.status = Some("follow off".to_owned());
            }
            KeyCode::Esc => {
                self.focus = Focus::List;
            }
            // `v` starts/stops a visual-line selection anchored at the
            // cursor; j/k then extend it (and the view follows).
            KeyCode::Char('v') => self.toggle_visual_select(),
            // j/k move a line cursor (vim-style); the view follows it. `y`
            // (handled globally, in content focus) yanks the cursor line.
            KeyCode::Char('j') | KeyCode::Down => self.move_cursor(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_cursor(-1),
            KeyCode::Char(' ') | KeyCode::PageDown => self.move_cursor(page),
            KeyCode::PageUp => self.move_cursor(-page),
            KeyCode::Char('g') | KeyCode::Home => self.cursor_to_end(false),
            KeyCode::Char('G') | KeyCode::End => self.cursor_to_end(true),
            // Arrows switch tabs (handled globally); h/l pan diagrams.
            KeyCode::Char('h') => self.hscroll = self.hscroll.saturating_sub(4),
            KeyCode::Char('l') => self.hscroll = self.hscroll.saturating_add(4),
            _ => {}
        }
    }

    pub(super) fn cycle_diagram(&mut self, direction: i8) {
        let total = match self.pane() {
            Some(Pane::Diagram { total, .. }) => *total,
            _ => return,
        };
        if total == 0 {
            return;
        }
        let total_i = isize::try_from(total).unwrap_or(isize::MAX);
        let current = isize::try_from(self.diagram_index).unwrap_or(0);
        let next = (current + isize::from(direction)).rem_euclid(total_i);
        self.diagram_index = usize::try_from(next).unwrap_or(0);
        self.reset_scroll();
    }
}
