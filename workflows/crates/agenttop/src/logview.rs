//! Interacting with the log view: panning it, and searching it.
//!
//! Split out of [`crate::app`] to keep that file inside the 500-line limit;
//! it is the same inherent impl on [`App`].
//!
//! The one design point worth knowing: matching runs over the SAME rendered
//! lines the view draws ([`crate::logs::plain_lines`] mirrors
//! [`crate::logs::build_lines`]), so a hit's index is the row on screen.
//! Searching the raw events instead would point at the wrong row the moment
//! one event expands into several lines, which most of them do.

use crate::app::{App, LogView, LOG_EVENT_LIMIT};

impl App {
    /// Open the event log of the run under the cursor.
    pub fn open_logs(&mut self) {
        let Some(run) = self.selected_run().cloned() else {
            self.set_status("nothing selected");
            return;
        };
        self.open_logs_for(run);
    }

    /// Logs for a specific run. The DAG page needs this: the step the
    /// cursor is on there is not the run the main list has selected.
    pub fn open_logs_for(&mut self, run: agent_core::types::Run) {
        match self.with_store(|s| s.events_after(&run.id, 0, LOG_EVENT_LIMIT)) {
            Ok(events) => {
                self.log_view = Some(LogView {
                    run,
                    events,
                    scroll_from_bottom: 0,
                    scroll_x: 0,
                    search: None,
                    search_input: false,
                    search_at: None,
                    search_line: None,
                });
            }
            Err(e) => self.set_status(&format!("load logs failed: {e}")),
        }
    }

    pub fn close_logs(&mut self) {
        self.log_view = None;
    }

    /// True while a search term is being typed, so the key loop sends
    /// keystrokes here instead of treating them as commands.
    pub fn search_typing(&self) -> bool {
        self.log_view.as_ref().is_some_and(|v| v.search_input)
    }

    /// `/` — start typing a term. Begins empty, replacing any previous one.
    pub fn search_begin(&mut self) {
        if let Some(view) = &mut self.log_view {
            view.search = Some(String::new());
            view.search_input = true;
        }
    }

    /// A keystroke while typing: enter commits, esc cancels, backspace
    /// deletes, anything printable appends. Lowercased, because matching is
    /// case-insensitive and doing it once here beats doing it per line.
    pub fn search_key(&mut self, code: crossterm::event::KeyCode) {
        use crossterm::event::KeyCode;
        let Some(view) = &mut self.log_view else {
            return;
        };
        match code {
            KeyCode::Enter => {
                view.search_input = false;
                view.search_at = None;
                view.search_line = None;
                // An empty term is the same as no search at all.
                if view.search.as_deref().is_some_and(str::is_empty) {
                    view.search = None;
                }
            }
            KeyCode::Esc => {
                view.search_input = false;
                view.search = None;
                view.search_at = None;
                view.search_line = None;
            }
            KeyCode::Backspace => {
                if let Some(s) = &mut view.search {
                    s.pop();
                }
            }
            KeyCode::Char(c) => {
                if let Some(s) = &mut view.search {
                    s.extend(c.to_lowercase());
                }
            }
            _ => {}
        }
    }

    /// n / N — move to the next (or previous) match and bring it on screen.
    ///
    /// Position is tracked as an index into the hit list, not inferred from
    /// the scroll offset. Inferring breaks whenever two hits share a
    /// screenful: the viewport barely moves and `n` looks broken even though
    /// it advanced. Wraps at both ends, like every pager.
    ///
    /// `n` is forward — toward later events, down the log.
    pub fn search_step(&mut self, frame_width: u16, frame_height: u16, forward: bool) {
        let Some(view) = &mut self.log_view else {
            return;
        };
        let Some(needle) = view.search.clone().filter(|t| !t.is_empty()) else {
            return;
        };
        // Exactly what draw_body uses — see body_metrics for why this must match.
        let (width, visible) = crate::logs::body_metrics(frame_width, frame_height);
        let lines = crate::logs::plain_lines(&view.events, width, view.scroll_x == 0);
        let hits: Vec<usize> = lines
            .iter()
            .enumerate()
            .filter(|(_, l)| l.to_lowercase().contains(&needle))
            .map(|(i, _)| i)
            .collect();
        if hits.is_empty() {
            view.search_at = None;
            view.search_line = None;
            return;
        }

        let next = match view.search_at {
            // Already on a hit: step, wrapping at either end.
            Some((pos, total)) if total == hits.len() => {
                let i = pos - 1; // stored 1-based for display
                if forward {
                    (i + 1) % hits.len()
                } else {
                    (i + hits.len() - 1) % hits.len()
                }
            }
            // First step after a new search (or the event list grew): start
            // from whatever is nearest the current viewport.
            _ => {
                let max_scroll = lines.len().saturating_sub(visible);
                let scroll = view.scroll_from_bottom.min(max_scroll);
                let top = lines.len().saturating_sub(scroll + visible);
                if forward {
                    hits.iter().position(|&h| h >= top).unwrap_or(0)
                } else {
                    hits.iter()
                        .rposition(|&h| h <= top)
                        .unwrap_or(hits.len() - 1)
                }
            }
        };

        let target = hits[next];
        view.search_at = Some((next + 1, hits.len()));
        view.search_line = Some(target);
        // Put the hit a few lines down from the top, so there is context above it.
        let margin = (visible / 4).min(5);
        let top = target.saturating_sub(margin);
        let max_scroll = lines.len().saturating_sub(visible);
        view.scroll_from_bottom = lines.len().saturating_sub(top + visible).min(max_scroll);
    }

    /// Pan the log view horizontally. Any pan disables wrapping; panning all
    /// the way back to column 0 re-enables it.
    pub fn pan_logs(&mut self, right: bool) {
        const PAN_STEP: u16 = 8;
        const PAN_MAX: u16 = 2000;
        let Some(view) = &mut self.log_view else {
            return;
        };
        if right {
            view.scroll_x = view.scroll_x.saturating_add(PAN_STEP).min(PAN_MAX);
        } else {
            view.scroll_x = view.scroll_x.saturating_sub(PAN_STEP);
        }
    }
}
