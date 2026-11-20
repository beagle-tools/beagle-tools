//! Shared visual vocabulary: severity badges, status glyphs, PR colors,
//! bordered blocks, and small layout helpers.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders};

use crate::model::{Severity, Status};

/// Default width of the incident list on the left (`w` resizes it).
pub(crate) const SIDEBAR_WIDTH: u16 = 34;

/// Orange for [`Status::Fixing`]. The 16-color palette has no orange, so
/// this is xterm-256 index 208 — near-universally supported.
const FIXING_ORANGE: Color = Color::Indexed(208);

pub(super) fn severity_badge(severity: Severity) -> (&'static str, Style) {
    let bold = Modifier::BOLD;
    match severity {
        Severity::Critical => (
            "CRIT",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Red)
                .add_modifier(bold),
        ),
        Severity::High => (
            "HIGH",
            Style::default()
                .fg(Color::Black)
                .bg(Color::LightRed)
                .add_modifier(bold),
        ),
        Severity::Medium => (
            "MED ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(bold),
        ),
        Severity::Low => (
            "LOW ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Green)
                .add_modifier(bold),
        ),
        Severity::Info => (
            "INFO",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Gray)
                .add_modifier(bold),
        ),
    }
}

/// The one-cell status marker. `investigating` and `fixing` animate — a live
/// spinner is the "we're on it" signal — driven by `tick`, told apart by
/// color (red = still debugging, orange = writing the fix); every other
/// status is a fixed glyph, so `tick` is ignored for them.
pub(super) fn status_symbol(status: Status, tick: usize) -> (&'static str, Style) {
    match status {
        Status::Investigating => (
            beagle_tui::widgets::spinner(tick),
            Style::default().fg(Color::LightRed),
        ),
        Status::Fixing => (
            beagle_tui::widgets::spinner(tick),
            Style::default().fg(FIXING_ORANGE),
        ),
        Status::Review => ("◐", Style::default().fg(Color::Yellow)),
        Status::Agent => ("⚙", Style::default().fg(Color::Magenta)),
        Status::FinalReview => ("◒", Style::default().fg(Color::LightBlue)),
        Status::Finished => ("✔", Style::default().fg(Color::Green)),
    }
}

/// Display color for a PR state: merged = done, open = attention,
/// draft = quiet, closed-unmerged = warning.
pub(super) fn pr_color(state: crate::prs::PrState) -> Color {
    use crate::prs::PrState;
    match state {
        PrState::Merged => Color::Green,
        PrState::Open => Color::Yellow,
        PrState::Draft => Color::Gray,
        PrState::Closed => Color::LightRed,
    }
}

pub(super) fn pane_block(title: String, focused: bool) -> Block<'static> {
    let border = if focused {
        Color::Yellow
    } else {
        Color::DarkGray
    };
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border))
}

/// Char-safe truncation to at most `max` chars, ending in `…` when cut —
/// the shared helper, re-exported under the short name the views use.
pub(super) use beagle_utils::text::truncate_chars as truncate;

pub(super) fn vertical<const N: usize>(area: Rect, constraints: &[Constraint; N]) -> [Rect; N] {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .areas(area)
}

pub(super) fn horizontal<const N: usize>(area: Rect, constraints: &[Constraint; N]) -> [Rect; N] {
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .areas(area)
}

pub(super) fn inset(area: Rect, left: u16) -> Rect {
    Rect {
        x: area.x.saturating_add(left),
        width: area.width.saturating_sub(left),
        ..area
    }
}

pub(super) fn center(area: Rect, width: u16, height: u16) -> Rect {
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    }
}

#[cfg(test)]
#[path = "tests/style.rs"]
mod tests;
