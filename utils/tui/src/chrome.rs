//! The two chrome pieces every beagle TUI shares: the `?` key-help sheet
//! and the one-line footer under the panes. One look in every tool:
//! yellow keys in a rounded yellow box, a dim key-hint footer, and status
//! messages coloured by [`Tone`].

use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, List, ListItem, Paragraph};
use ratatui::Frame;

use crate::fuzzy;
use crate::pane_resize;
use crate::widgets::{centered, truncate};

/// The help sheet: `(keys, what they do)` rows, optionally narrowed by a
/// fuzzy `filter` (rca's `f` inside help). `None` means the app offers no
/// filter; `Some("")` means a filter is available but empty.
#[derive(Debug, Clone, Copy)]
pub enum HelpFilter<'a> {
    /// The app has no help filter: the box says "any key closes".
    Off,
    /// A filter is available (`f` starts it) but not typed.
    Available,
    /// The user is typing this query; rows narrow live.
    Typing(&'a str),
}

/// Draws the key-help overlay centred on the frame. The box keeps the
/// height of the full sheet, so filtering never resizes it.
pub fn draw_help(frame: &mut Frame<'_>, help: &[(&str, &str)], filter: HelpFilter<'_>) {
    let area = frame.area();
    let key_col = help
        .iter()
        .map(|(k, _)| k.chars().count())
        .max()
        .unwrap_or(0)
        + 2;
    let needed = help
        .iter()
        .map(|(_, a)| 4 + key_col + a.chars().count())
        .max()
        .unwrap_or(40);
    let width = u16::try_from(needed)
        .unwrap_or(u16::MAX)
        .clamp(40, area.width.saturating_sub(4).max(40));
    let height = u16::try_from(help.len())
        .unwrap_or(u16::MAX)
        .saturating_add(2)
        .min(area.height.saturating_sub(2));
    let popup = centered(area, width, height);
    let action_room = usize::from(width).saturating_sub(4 + key_col);
    let query = match filter {
        HelpFilter::Typing(q) => Some(q),
        HelpFilter::Off | HelpFilter::Available => None,
    };
    let mut items: Vec<ListItem<'_>> = help_rows(help, query)
        .into_iter()
        .map(|(keys, action)| {
            ListItem::new(Line::from(row_spans(
                keys,
                &truncate(action, action_room),
                key_col,
                query,
            )))
        })
        .collect();
    if items.is_empty() {
        items.push(ListItem::new(Line::styled(
            "  (no matching keys)",
            Style::default().fg(Color::DarkGray),
        )));
    }
    let bottom = match filter {
        HelpFilter::Off => " any key closes ".to_owned(),
        HelpFilter::Available => " f filter · any key closes ".to_owned(),
        HelpFilter::Typing(q) => format!(" filter: {q}▌ · esc clears "),
    };
    let block = Block::default()
        .title(" keys ")
        .title_alignment(Alignment::Center)
        .title_bottom(Line::from(bottom).centered())
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Yellow));
    frame.render_widget(Clear, popup);
    frame.render_widget(List::new(items).block(block), popup);
}

/// The rows a `query` keeps (all of them when it is `None` or empty).
#[must_use]
pub fn help_rows<'h>(help: &[(&'h str, &'h str)], query: Option<&str>) -> Vec<(&'h str, &'h str)> {
    help.iter()
        .copied()
        .filter(|(keys, action)| match query {
            Some(q) if !q.is_empty() => fuzzy::score(q, &format!("{keys} {action}")).is_some(),
            _ => true,
        })
        .collect()
}

/// One help row (`  <keys padded><action>`): keys yellow, and the
/// characters a filter matched highlighted so it is clear why a row stayed.
fn row_spans(keys: &str, action: &str, key_col: usize, query: Option<&str>) -> Vec<Span<'static>> {
    let text = format!("  {keys:<key_col$}{action}");
    let keys_end = 2 + keys.chars().count();
    let matched: Vec<usize> = match query {
        Some(q) if !q.is_empty() => fuzzy::indices(q, &text)
            .map(|(_, positions)| positions)
            .unwrap_or_default(),
        _ => Vec::new(),
    };
    // 0 = matched (highlight), 1 = keys (yellow), 2 = normal.
    let class = |i: usize| -> u8 {
        if matched.contains(&i) {
            0
        } else if (2..keys_end).contains(&i) {
            1
        } else {
            2
        }
    };
    let style_of = |c: u8| match c {
        0 => Style::default()
            .fg(Color::Rgb(214, 160, 30))
            .add_modifier(Modifier::BOLD),
        1 => Style::default().fg(Color::Yellow),
        _ => Style::default(),
    };
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut buf = String::new();
    let mut buf_class = 2u8;
    for (i, ch) in text.chars().enumerate() {
        let c = class(i);
        if !buf.is_empty() && c != buf_class {
            spans.push(Span::styled(std::mem::take(&mut buf), style_of(buf_class)));
        }
        buf_class = c;
        buf.push(ch);
    }
    if !buf.is_empty() {
        spans.push(Span::styled(buf, style_of(buf_class)));
    }
    spans
}

/// What a footer status message is, which sets its colour and mark.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    /// A neutral note (yellow).
    Info,
    /// Something worked (green, `✓`).
    Ok,
    /// Something failed (red, `✗`).
    Error,
    /// Work in progress (cyan, `⟳`).
    Busy,
}

/// The footer line: while resizing, the resize hint (and any status beside
/// it); else a status message, which gets the whole line; else the dim key
/// `hints`. Apps may push extra spans (a FOLLOW badge, a warning) onto it.
#[must_use]
pub fn footer_line(hints: &str, status: Option<(&str, Tone)>, resizing: bool) -> Line<'static> {
    if resizing {
        let mut spans = vec![Span::styled(
            format!(" {}", pane_resize::HINT),
            Style::default().fg(Color::Cyan),
        )];
        if let Some((message, _)) = status {
            spans.push(Span::styled(
                format!("   {message}"),
                Style::default().fg(Color::Yellow),
            ));
        }
        return Line::from(spans);
    }
    match status {
        Some((message, tone)) => {
            let (mark, color) = match tone {
                Tone::Info => ("", Color::Yellow),
                Tone::Ok => ("✓ ", Color::LightGreen),
                Tone::Error => ("✗ ", Color::LightRed),
                Tone::Busy => ("⟳ ", Color::Cyan),
            };
            Line::styled(format!(" {mark}{message}"), Style::default().fg(color))
        }
        None => Line::styled(format!(" {hints}"), Style::default().fg(Color::DarkGray)),
    }
}

/// Renders [`footer_line`] into `area`.
pub fn draw_footer(
    frame: &mut Frame<'_>,
    area: Rect,
    hints: &str,
    status: Option<(&str, Tone)>,
    resizing: bool,
) {
    frame.render_widget(Paragraph::new(footer_line(hints, status, resizing)), area);
}

#[cfg(test)]
#[path = "tests/chrome.rs"]
mod tests;
