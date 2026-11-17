//! Drawing pieces every app's view uses: pane borders, centred overlays,
//! the key-help sheet, and the document pane with its line cursor.

use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::docview::DocView;

/// The highlight behind the selected row of a list.
pub const LIST_HIGHLIGHT: Color = Color::Rgb(40, 44, 52);
/// The line cursor's bar.
const CURSOR_BG: Color = Color::Rgb(55, 60, 72);
/// A visual selection's bar.
const SELECTION_BG: Color = Color::Rgb(92, 64, 30);

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

/// The key-help overlay: one `(keys, what they do)` row per entry.
pub fn draw_help(frame: &mut Frame<'_>, help: &[(&str, &str)]) {
    let key_width = help
        .iter()
        .map(|(k, _)| k.chars().count())
        .max()
        .unwrap_or(0);
    let row_width = help
        .iter()
        .map(|(_, d)| key_width + 3 + d.chars().count())
        .max()
        .unwrap_or(0);
    let rect = centered(
        frame.area(),
        u16::try_from(row_width + 4).unwrap_or(u16::MAX),
        u16::try_from(help.len() + 2).unwrap_or(u16::MAX),
    );
    let lines: Vec<Line<'static>> = help
        .iter()
        .map(|(k, d)| {
            Line::from(vec![
                Span::styled(
                    format!(" {k:<key_width$}  "),
                    Style::default().fg(Color::Cyan),
                ),
                Span::raw((*d).to_owned()),
            ])
        })
        .collect();
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" keys · any key closes "),
        ),
        rect,
    );
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
