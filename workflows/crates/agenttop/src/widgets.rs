//! Small drawing helpers every jobs screen shares: tree guides, the
//! scrolling window, badges, hints, pane borders and state colours.
//!
//! Split from `dag.rs` (the screens themselves); re-exported there.

use agent_core::types::RunState;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Block;

use crate::app::App;

/// Box-drawing guides for rows given in depth-first order, row 0 being
/// the root (which gets none). `├─` for a node with a later sibling,
/// `└─` for the last, and `│` carried down for every ancestor that still
/// has siblings to come.
pub(crate) fn tree_prefixes(depths: &[usize]) -> Vec<String> {
    let mut out = Vec::with_capacity(depths.len());
    // more[k]: the ancestor at depth k+1 has later siblings, so its
    // column needs a `│` running down past this row.
    let mut more: Vec<bool> = Vec::new();
    for (i, &depth) in depths.iter().enumerate() {
        if depth == 0 {
            more.clear();
            out.push(String::new());
            continue;
        }
        more.truncate(depth - 1);
        let last = depths[i + 1..]
            .iter()
            .find(|&&d| d <= depth)
            .is_none_or(|&d| d < depth);
        let mut prefix = String::new();
        for &open in &more {
            prefix.push_str(if open { "│  " } else { "   " });
        }
        prefix.push_str(if last { "└─ " } else { "├─ " });
        more.push(!last);
        out.push(prefix);
    }
    out
}

/// The slice of `lines` that fits `area`, scrolled just enough to keep
/// `selected` on screen.
pub(crate) fn window(lines: Vec<Line<'_>>, selected: usize, area: Rect) -> Vec<Line<'_>> {
    let visible = usize::from(area.height.saturating_sub(2)).max(1);
    let start = selected.saturating_sub(visible.saturating_sub(2));
    lines.into_iter().skip(start).take(visible).collect()
}

pub(crate) fn badge(text: &str) -> Span<'static> {
    Span::styled(
        text.to_owned(),
        Style::new()
            .fg(Color::Black)
            .bg(Color::Magenta)
            .add_modifier(Modifier::BOLD),
    )
}

pub(crate) fn hints(app: &App, default: &'static str) -> Span<'static> {
    match app.status_message() {
        Some(status) => Span::styled(
            format!(" {status} "),
            Style::new().fg(Color::Magenta).add_modifier(Modifier::BOLD),
        ),
        None => Span::styled(default, Style::new().add_modifier(Modifier::DIM)),
    }
}

pub(crate) fn pane_block(title: &str) -> Block<'static> {
    Block::bordered().title(Span::styled(
        format!(" {title} "),
        Style::new().add_modifier(Modifier::BOLD),
    ))
}

pub(crate) fn state_style(state: RunState) -> Style {
    match state {
        RunState::Running => Style::new().fg(Color::Green),
        RunState::Pending => Style::new().fg(Color::Yellow),
        RunState::Succeeded => Style::new().fg(Color::Blue),
        RunState::Failed => Style::new().fg(Color::Red),
        RunState::TimedOut => Style::new().fg(Color::Magenta),
        RunState::Killed => Style::new().fg(Color::DarkGray),
    }
}
