//! Resizing the PR list: `w` enters resize mode, ←/→ (or h/l) narrow and
//! widen the list, Esc/Enter/w leave it.
//!
//! The width is bounded both ways at every terminal size: the list never
//! drops below [`MIN_LIST`] columns (PR titles stop being readable) and the
//! detail pane always keeps [`MIN_CONTENT`] (a diff line needs the room).
//! On a terminal too narrow for both, the list wins its minimum.

use crossterm::event::{KeyCode, KeyEvent};

use super::{App, Mode};

/// The narrowest the list may get.
pub const MIN_LIST: u16 = 20;
/// The widest the list may get, however wide the terminal.
pub const MAX_LIST: u16 = 100;
/// Columns the detail pane always keeps.
pub const MIN_CONTENT: u16 = 40;
/// Columns one keypress moves the divider.
pub const STEP: u16 = 2;

/// The largest list width that still leaves the detail pane its minimum.
fn upper(total: u16) -> u16 {
    total.saturating_sub(MIN_CONTENT).clamp(MIN_LIST, MAX_LIST)
}

/// The list width to draw: the chosen one if set, else a third of the
/// screen (30–56 columns), always within the limits for `total`.
pub fn list_width(chosen: Option<u16>, total: u16) -> u16 {
    let width = chosen.unwrap_or_else(|| (total / 3).clamp(30, 56));
    width.clamp(MIN_LIST, upper(total))
}

impl App {
    /// A key while resizing. Anything but the arrows and the exit keys is
    /// ignored, so a stray press can't do something else by surprise.
    pub(super) fn resize_key(&mut self, key: KeyEvent) {
        let total = self.main_width;
        let now = list_width(self.list_width, total);
        let next = match key.code {
            KeyCode::Left | KeyCode::Char('h') => now.saturating_sub(STEP),
            KeyCode::Right | KeyCode::Char('l') => now.saturating_add(STEP),
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('w' | 'q') => {
                self.info(format!("list width {now}"));
                return;
            }
            _ => {
                self.mode = Mode::Resize;
                return;
            }
        };
        let clamped = next.clamp(MIN_LIST, upper(total));
        self.list_width = Some(clamped);
        self.mode = Mode::Resize;
        if clamped == now {
            let edge = if next < now { "narrowest" } else { "widest" };
            self.info(format!("list is at its {edge} ({now} columns)"));
        } else {
            self.status = None;
        }
    }

    /// `w`: start resizing, unless the list is zoomed away.
    pub(super) fn start_resize(&mut self) {
        if self.zoom {
            self.info("unzoom (z) to resize the list");
        } else {
            self.mode = Mode::Resize;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_is_a_third_within_30_to_56() {
        assert_eq!(list_width(None, 120), 40);
        assert_eq!(list_width(None, 300), 56);
        assert_eq!(list_width(None, 80), 30);
    }

    #[test]
    fn a_chosen_width_is_held_to_both_limits() {
        assert_eq!(list_width(Some(5), 200), MIN_LIST);
        assert_eq!(list_width(Some(500), 300), MAX_LIST);
        // The detail pane keeps its minimum on a mid-size screen.
        assert_eq!(list_width(Some(90), 100), 100 - MIN_CONTENT);
    }

    #[test]
    fn a_tiny_terminal_still_gets_the_minimum_list() {
        assert_eq!(list_width(Some(50), 30), MIN_LIST);
        assert_eq!(list_width(None, 10), MIN_LIST);
    }
}
