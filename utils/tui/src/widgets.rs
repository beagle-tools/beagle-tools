//! Drawing pieces every app's view uses: pane borders, centred overlays,
//! and the document pane with its line cursor. The help sheet and footer
//! are in [`crate::chrome`].

use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

use crate::docview::DocView;

/// The highlight behind the selected row of a list.
pub const LIST_HIGHLIGHT: Color = Color::Rgb(40, 44, 52);
/// The line cursor's bar.
const CURSOR_BG: Color = Color::Rgb(55, 60, 72);
/// A visual selection's bar.
const SELECTION_BG: Color = Color::Rgb(92, 64, 30);

/// The braille spinner every TUI shows for work in progress ("an agent is
/// on it right now"). Each frame is exactly one cell wide, so rows never
/// shift as it animates.
pub const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// The [`SPINNER`] frame for a redraw `tick` (wraps around).
#[must_use]
pub fn spinner(tick: usize) -> &'static str {
    SPINNER[tick % SPINNER.len()]
}

/// A pane border: cyan when focused, dim otherwise.
#[must_use]
pub fn border_style(focused: bool) -> Style {
    if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    }
}

/// A `width` × `height` rectangle centred in `area` (clamped to it).
#[must_use]
pub fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    }
}

/// Draws a document pane: `lines` (normally [`DocView::visible`]) inside
/// `block`, panned by the view. Records the pane width and clamps the pan.
pub fn draw_doc(
    frame: &mut Frame<'_>,
    area: Rect,
    block: Block<'_>,
    view: &mut DocView,
    lines: Vec<Line<'static>>,
) {
    view.content_width = block.inner(area).width;
    view.pan = view.pan.min(view.max_pan());
    frame.render_widget(
        Paragraph::new(lines).block(block).scroll((0, view.pan)),
        area,
    );
}

/// Paints the line cursor and visual selection as full-width bars onto the
/// buffer (so blank lines highlight too). `area` is the pane, `inner` its
/// inside; call it only while the pane has focus.
pub fn paint_cursor(frame: &mut Frame<'_>, area: Rect, inner: Rect, view: &DocView) {
    if view.laid.is_empty() {
        return;
    }
    let (first, last) = view.selection();
    let top = usize::from(view.scroll);
    let bar = Rect {
        x: area.x.saturating_add(1),
        width: area.width.saturating_sub(2),
        height: 1,
        y: 0,
    };
    for row in first.max(top)..=last.min(top + usize::from(inner.height).saturating_sub(1)) {
        let y = inner.y + u16::try_from(row - top).unwrap_or(0);
        let bg = if row == view.cursor {
            CURSOR_BG
        } else {
            SELECTION_BG
        };
        frame
            .buffer_mut()
            .set_style(Rect { y, ..bar }, Style::default().bg(bg));
    }
}

/// `s` cut to `max` characters, ending in `…` when cut.
#[must_use]
pub fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_owned()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

#[cfg(test)]
#[path = "tests/widgets.rs"]
mod tests;
