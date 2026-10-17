//! The content pane: the current tab's pre-rendered text with the line
//! cursor, visual selection, and search-match highlights layered on top.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Text};
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::Frame;

use crate::ui::{App, Focus, Pane};

use super::style::pane_block;

pub(super) fn draw_content(frame: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.focus() == Focus::Content;
    let (scroll, hscroll) = app.scroll_offsets();

    let (text, wrapped, title): (&Text<'static>, bool, String) = match app.pane() {
        Some(Pane::Section { text, .. }) => (text, true, format!(" {} ", app.tab().title())),
        Some(Pane::Diagram {
            text,
            name,
            index,
            total,
            ..
        }) => (
            text,
            false,
            format!(" {name}  [{}/{total}]  n/p to cycle ", index + 1),
        ),
        Some(Pane::Empty(hint)) => {
            let paragraph = Paragraph::new(hint.clone())
                .style(Style::default().fg(Color::DarkGray))
                .wrap(Wrap { trim: false })
                .block(pane_block(format!(" {} ", app.tab().title()), focused));
            frame.render_widget(paragraph, area);
            return;
        }
        Some(Pane::LoadError(message)) => {
            let paragraph = Paragraph::new(message.clone())
                .style(Style::default().fg(Color::Red))
                .wrap(Wrap { trim: false })
                .block(pane_block(" load error ".to_owned(), focused));
            frame.render_widget(paragraph, area);
            return;
        }
        None => return,
    };

    let block = pane_block(title, focused);
    let inner = block.inner(area);

    let mut text = text.clone();
    highlight_search_matches(&mut text, app);

    // Total wrapped height feeds scroll clamping; summing per-line rows
    // avoids consuming `text` (we still need it for the cursor bar).
    let content_lines = if wrapped {
        crate::ui::search::wrapped_rows_before(&text, text.lines.len(), inner.width)
    } else {
        u16::try_from(text.lines.len()).unwrap_or(u16::MAX)
    };
    app.viewport = crate::ui::ViewportInfo {
        content_lines,
        height: inner.height,
        width: inner.width,
    };
    let scroll = scroll.min(content_lines.saturating_sub(inner.height));

    // The cursor/selection bar is painted onto the buffer *after* the text,
    // not baked into it: a full-width row background can't be faked with
    // trailing spaces (an all-whitespace line wraps to a second row).
    let bars = cursor_highlight_rects(app, &text, inner, scroll, focused, wrapped);

    let mut paragraph = Paragraph::new(text);
    if wrapped {
        paragraph = paragraph.wrap(Wrap { trim: false });
    }
    let paragraph = paragraph
        .block(block)
        .scroll((scroll, if wrapped { 0 } else { hscroll }));
    frame.render_widget(paragraph, area);

    for (rect, bg) in bars {
        // Patches only the background, so text and its colours survive.
        frame.buffer_mut().set_style(rect, Style::default().bg(bg));
    }
}

/// Highlights the matched text itself on the visible tab — the occurrence,
/// not the whole line, so the eye lands exactly on it. The current hit's
/// occurrences pop in amber-on-black; other hits get a quieter steel tint.
/// Tints the content line cursor — or the whole visual-line selection —
/// while the pane is focused, so `y` and j/k navigation have a visible
/// anchor. Each highlighted row is padded to the full width, so blank
/// lines show the bar too. Search highlights are applied afterwards and
/// win on the same line.
/// The full-width row rectangles to paint for the content cursor — or the
/// whole visual-line selection — with their colour. Each source line can
/// span several wrapped screen rows; every visible row of a highlighted
/// line is included. Empty when the pane isn't focused.
pub(super) fn cursor_highlight_rects(
    app: &App,
    text: &Text<'static>,
    inner: Rect,
    scroll: u16,
    focused: bool,
    wrapped: bool,
) -> Vec<(Rect, Color)> {
    if !focused {
        return Vec::new();
    }
    // Visual selection is a warm orange; the plain cursor a subtle bar.
    let (lo, hi, bg) = if let Some((lo, hi)) = app.selection_lines() {
        (lo, hi, Color::Rgb(150, 100, 45))
    } else {
        let c = app.content_cursor();
        (c, c, Color::Rgb(48, 52, 70))
    };
    let row_of = |idx: usize| -> u16 {
        if wrapped {
            crate::ui::search::wrapped_rows_before(text, idx, inner.width)
        } else {
            u16::try_from(idx).unwrap_or(u16::MAX)
        }
    };
    let mut rects = Vec::new();
    for idx in lo..=hi {
        let (top, bottom) = (row_of(idx), row_of(idx + 1));
        for visual in top..bottom {
            if visual >= scroll && visual < scroll.saturating_add(inner.height) {
                rects.push((
                    Rect {
                        x: inner.x,
                        y: inner.y + (visual - scroll),
                        width: inner.width,
                        height: 1,
                    },
                    bg,
                ));
            }
        }
    }
    rects
}

pub(super) fn highlight_search_matches(text: &mut Text<'static>, app: &App) {
    let Some(query) = app.content_search().map(|s| s.query.clone()) else {
        return;
    };
    for (line_index, is_current) in app.search_highlights(app.tab()) {
        if let Some(line) = text.lines.get_mut(line_index) {
            let style = if is_current {
                Style::default()
                    .bg(Color::Rgb(214, 160, 30))
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().bg(Color::Rgb(60, 68, 96))
            };
            *line = crate::ui::search::highlight_occurrences(line, &query, style);
        }
    }
}

pub(super) fn draw_welcome(frame: &mut Frame, area: Rect) {
    let text = Text::from(vec![
        Line::from(""),
        Line::styled(
            "  no RCA workspaces yet",
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Line::from(""),
        Line::from("  Create one:"),
        Line::styled(
            "    beagle new my-incident --title \"What broke\" --severity high",
            Style::default().fg(Color::LightGreen),
        ),
        Line::from(""),
        Line::from("  …or ask Claude to debug a system; it will scaffold a workspace"),
        Line::from("  under rcas/ and this view will pick it up live."),
    ]);
    let block = pane_block(" beagle ".to_owned(), false);
    frame.render_widget(Paragraph::new(text).block(block), area);
}
