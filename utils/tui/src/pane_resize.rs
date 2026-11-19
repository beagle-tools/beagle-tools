//! The one "resize the side pane" interaction every beagle-tools TUI shares.
//!
//! `w` (from the app's normal state) calls [`SideResize::start`]; while
//! [`SideResize::is_active`], the app hands every key to
//! [`SideResize::handle_key`]:
//!
//! - `←`/`h` narrow the side pane and `→`/`l` widen it, by
//!   [`PaneLimits::step`] columns;
//! - `0` goes back to the app's default width;
//! - `Esc`, `Enter`, `w` and `q` leave resize mode (`q` never quits here);
//! - every other key is swallowed, so a stray press can't act by surprise.
//!
//! The width is bounded both ways at every terminal size: the side pane
//! never drops below [`PaneLimits::min`] or grows past [`PaneLimits::max`],
//! and the other pane always keeps [`PaneLimits::min_other`]. On a terminal
//! too narrow for both, the side pane keeps its minimum. The chosen width
//! lives only in memory: restarting the app returns to the default.

use std::cell::Cell;

use crossterm::event::{KeyCode, KeyEvent};

/// The footer hint to show while resizing.
pub const HINT: &str = "resize: ←/→ width · 0 default · Esc done";

/// The key-help row for `w`, for an app's `?` overlay.
pub const HELP: (&str, &str) = ("w", "resize the side pane: ←/→ h/l · 0 default · Esc done");

/// How wide the side pane is before anyone resizes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefaultWidth {
    /// Always this many columns.
    Fixed(u16),
    /// `total / divisor` columns, clamped to `min..=max`.
    Fraction {
        /// The screen is divided by this (3 = a third).
        divisor: u16,
        /// The default never drops below this.
        min: u16,
        /// The default never grows past this.
        max: u16,
    },
}

impl DefaultWidth {
    /// The default for a `total`-column screen, before the pane limits.
    fn columns(self, total: u16) -> u16 {
        match self {
            Self::Fixed(width) => width,
            Self::Fraction { divisor, min, max } => {
                (total / divisor.max(1)).clamp(min, max.max(min))
            }
        }
    }
}

/// The bounds every resize honours.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaneLimits {
    /// The narrowest the side pane may get.
    pub min: u16,
    /// The widest the side pane may get, however wide the terminal.
    pub max: u16,
    /// Columns the other (main/detail) pane always keeps.
    pub min_other: u16,
    /// Columns one keypress moves the divider.
    pub step: u16,
}

impl PaneLimits {
    /// The standard limits: 20–100 columns, 40 kept for the other pane,
    /// 2 columns a step.
    pub const STANDARD: Self = Self {
        min: 20,
        max: 100,
        min_other: 40,
        step: 2,
    };

    /// The largest side width that still leaves the other pane its minimum.
    fn upper(self, total: u16) -> u16 {
        total
            .saturating_sub(self.min_other)
            .clamp(self.min, self.max.max(self.min))
    }

    /// `width` held to both limits on a `total`-column screen.
    fn clamp(self, width: u16, total: u16) -> u16 {
        width.clamp(self.min, self.upper(total))
    }
}

/// What a key did while resizing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// The key left resize mode.
    pub left: bool,
    /// A short status for the footer, if the key earned one. `None` after
    /// an ordinary step, so the app can clear a stale message.
    pub status: Option<String>,
}

/// A side pane's width and its resize mode.
#[derive(Debug, Clone)]
pub struct SideResize {
    label: &'static str,
    default: DefaultWidth,
    limits: PaneLimits,
    chosen: Option<u16>,
    active: bool,
    last_total: Cell<u16>,
}

impl SideResize {
    /// A pane called `label` in status messages ("list", "sidebar", …)
    /// starting at `default`, with [`PaneLimits::STANDARD`].
    #[must_use]
    pub const fn new(label: &'static str, default: DefaultWidth) -> Self {
        Self::with_limits(label, default, PaneLimits::STANDARD)
    }

    /// As [`SideResize::new`], with custom limits.
    #[must_use]
    pub const fn with_limits(
        label: &'static str,
        default: DefaultWidth,
        limits: PaneLimits,
    ) -> Self {
        Self {
            label,
            default,
            limits,
            chosen: None,
            active: false,
            last_total: Cell::new(0),
        }
    }

    /// The limits this pane honours.
    #[must_use]
    pub const fn limits(&self) -> PaneLimits {
        self.limits
    }

    /// The side pane's width on a `total`-column screen: the chosen width if
    /// any, else the default, always within the limits. Also remembers
    /// `total` (see [`SideResize::last_total`]) so a draw pass that only has
    /// `&self` still tells the key handler how wide the screen was.
    #[must_use]
    pub fn width(&self, total: u16) -> u16 {
        self.last_total.set(total);
        self.width_at(total)
    }

    /// The default width on a `total`-column screen, within the limits.
    #[must_use]
    pub fn default_width(&self, total: u16) -> u16 {
        self.limits.clamp(self.default.columns(total), total)
    }

    /// The width last asked for by [`SideResize::width`]'s `total`.
    #[must_use]
    pub fn last_total(&self) -> u16 {
        self.last_total.get()
    }

    /// The width without recording `total`.
    fn width_at(&self, total: u16) -> u16 {
        let width = self.chosen.unwrap_or_else(|| self.default.columns(total));
        self.limits.clamp(width, total)
    }

    /// Enter resize mode.
    pub fn start(&mut self) {
        self.active = true;
    }

    /// Leave resize mode without a key (e.g. the pane was hidden).
    pub fn stop(&mut self) {
        self.active = false;
    }

    /// Whether resize mode is on.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.active
    }

    /// Handle one key while resizing, on a `total`-column screen.
    pub fn handle_key(&mut self, key: KeyEvent, total: u16) -> Outcome {
        let now = self.width_at(total);
        let label = self.label;
        let next = match key.code {
            KeyCode::Left | KeyCode::Char('h') => now.saturating_sub(self.limits.step),
            KeyCode::Right | KeyCode::Char('l') => now.saturating_add(self.limits.step),
            KeyCode::Char('0') => {
                self.chosen = None;
                let width = self.width_at(total);
                return stay(Some(format!(
                    "{label} back to its default ({width} columns)"
                )));
            }
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('w' | 'q') => {
                self.active = false;
                return Outcome {
                    left: true,
                    status: Some(format!("{label} width {now}")),
                };
            }
            _ => return stay(None),
        };
        let clamped = self.limits.clamp(next, total);
        self.chosen = Some(clamped);
        if clamped == now {
            let edge = if next < now { "narrowest" } else { "widest" };
            stay(Some(format!("{label} is at its {edge} ({now} columns)")))
        } else {
            stay(None)
        }
    }
}

/// Still resizing.
const fn stay(status: Option<String>) -> Outcome {
    Outcome {
        left: false,
        status,
    }
}

#[cfg(test)]
#[path = "tests/pane_resize.rs"]
mod tests;
