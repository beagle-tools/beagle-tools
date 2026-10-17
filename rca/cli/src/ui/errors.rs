//! The `!` errors overlay: broken workspaces and load warnings, with
//! scrolling and copy-to-clipboard.

use std::fmt::Write as _;

use super::App;

impl App {
    /// Workspace directories that exist on disk but could not load.
    pub(crate) fn broken(&self) -> &[crate::store::BrokenWorkspace] {
        &self.broken
    }

    /// Whether there is anything for the `!` errors overlay to show.
    pub(crate) fn has_problems(&self) -> bool {
        !self.broken.is_empty() || !self.warnings.is_empty()
    }

    /// Opens the `!` errors overlay if there is anything to show; otherwise
    /// says so in the status bar rather than flashing an empty pane.
    pub(crate) fn open_errors(&mut self) {
        if self.has_problems() {
            self.show_errors = true;
            self.errors_scroll = 0;
        } else {
            self.status = Some("no load errors or warnings".to_owned());
        }
    }

    /// The overlay's contents as plain text, for the clipboard: each
    /// broken workspace as `⚠ <dir>` then its full reason, then any load
    /// warnings. Mirrors what [`draw_errors`](view) shows.
    pub(super) fn errors_plain_text(&self) -> String {
        let mut out = String::new();
        for broken in &self.broken {
            let _ = writeln!(out, "⚠ {}\n{}\n", broken.dir_name, broken.reason);
        }
        for warning in &self.warnings {
            let _ = writeln!(out, "warning: {}", warning.0);
        }
        out.trim_end().to_owned()
    }

    /// Keystrokes while the errors overlay is open: scroll, copy, or close.
    pub(crate) fn handle_errors_key(&mut self, code: crossterm::event::KeyCode) {
        use crossterm::event::KeyCode;
        let (content_lines, height) = self.errors_viewport;
        let max = content_lines.saturating_sub(height);
        let page = height.saturating_sub(1).max(1);
        match code {
            KeyCode::Esc | KeyCode::Char('q' | '!') => self.show_errors = false,
            KeyCode::Char('c') => {
                let text = self.errors_plain_text();
                self.finish_copy("load errors", &text);
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.errors_scroll = self.errors_scroll.saturating_add(1).min(max);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.errors_scroll = self.errors_scroll.saturating_sub(1);
            }
            KeyCode::Char(' ') | KeyCode::PageDown => {
                self.errors_scroll = self.errors_scroll.saturating_add(page).min(max);
            }
            KeyCode::PageUp => self.errors_scroll = self.errors_scroll.saturating_sub(page),
            KeyCode::Char('g') | KeyCode::Home => self.errors_scroll = 0,
            KeyCode::Char('G') | KeyCode::End => self.errors_scroll = max,
            _ => {}
        }
    }

    /// Whether the `!` errors overlay is open.
    pub(crate) fn errors_visible(&self) -> bool {
        self.show_errors
    }

    pub(crate) fn errors_scroll(&self) -> u16 {
        self.errors_scroll
    }
}
