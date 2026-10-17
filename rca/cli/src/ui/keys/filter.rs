//! Filter-mode keys: facet toggles and the free-text fuzzy query over the
//! incident list.

use crossterm::event::KeyCode;

use crate::ui::App;

impl App {
    /// Keystrokes in filter mode. Facets first: single keys toggle status
    /// (`i`/`r`/`v`/`f`) and severity (`c`/`h`/`m`/`l`) filters instantly,
    /// stacking across dimensions; `/` switches to free-text typing. Arrows
    /// (and `j`/`k` while not typing) still move the selection so filtering
    /// and picking interleave naturally.
    pub(super) fn handle_search_key(&mut self, code: KeyCode) {
        use crate::model::{Severity, Status};

        if self.filter_input == crate::ui::FilterInput::Typing {
            match code {
                // Esc peels: stop typing first (query kept), facets next.
                KeyCode::Esc => self.filter_input = crate::ui::FilterInput::Facets,
                KeyCode::Enter => self.filter_input = crate::ui::FilterInput::Off,
                KeyCode::Backspace => {
                    self.filter.pop();
                    self.recompute_visible(None);
                }
                KeyCode::Char(c) => {
                    self.filter.push(c);
                    self.recompute_visible(None);
                }
                KeyCode::Down => self.select(self.selected.saturating_add(1)),
                KeyCode::Up => self.select(self.selected.saturating_sub(1)),
                _ => {}
            }
            return;
        }
        match code {
            KeyCode::Esc => {
                self.clear_filter();
                self.filter_input = crate::ui::FilterInput::Off;
                self.reset_scroll();
            }
            KeyCode::Enter => self.filter_input = crate::ui::FilterInput::Off,
            KeyCode::Char('/') => self.filter_input = crate::ui::FilterInput::Typing,
            KeyCode::Char('i') => self.toggle_status_facet(Status::Investigating),
            KeyCode::Char('r') => self.toggle_status_facet(Status::Review),
            KeyCode::Char('a') => self.toggle_status_facet(Status::Agent),
            KeyCode::Char('v') => self.toggle_status_facet(Status::FinalReview),
            KeyCode::Char('f') => self.toggle_status_facet(Status::Finished),
            KeyCode::Char('c') => self.toggle_severity_facet(Severity::Critical),
            KeyCode::Char('h') => self.toggle_severity_facet(Severity::High),
            KeyCode::Char('m') => self.toggle_severity_facet(Severity::Medium),
            KeyCode::Char('l') => self.toggle_severity_facet(Severity::Low),
            KeyCode::Char('p') => {
                self.facet_has_pr = !self.facet_has_pr;
                self.recompute_visible(None);
            }
            KeyCode::Char('s') => {
                self.facet_security = !self.facet_security;
                self.recompute_visible(None);
            }
            KeyCode::Backspace => {
                self.filter.pop();
                self.recompute_visible(None);
            }
            KeyCode::Char('j') | KeyCode::Down => self.select(self.selected.saturating_add(1)),
            KeyCode::Char('k') | KeyCode::Up => self.select(self.selected.saturating_sub(1)),
            _ => {}
        }
    }

    pub(super) fn toggle_status_facet(&mut self, status: crate::model::Status) {
        if !self.facet_statuses.remove(&status) {
            self.facet_statuses.insert(status);
        }
        self.recompute_visible(None);
    }

    pub(super) fn toggle_severity_facet(&mut self, severity: crate::model::Severity) {
        if !self.facet_severities.remove(&severity) {
            self.facet_severities.insert(severity);
        }
        self.recompute_visible(None);
    }
}
