//! The `\` global fuzzy finder popup: query line plus ranked hits across
//! every workspace's sections.

use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState};
use ratatui::Frame;

use crate::ui::App;

use super::super::style::{center, truncate};

/// The `\` global finder: prompt on top, ranked results under it, matched
/// characters highlighted. Centered, and sized to its content: a slim
/// bar while the query is empty, one row per match as results arrive,
/// capped well short of the full screen — the incident stays visible
/// behind it.
pub(in crate::ui::view) fn draw_finder(frame: &mut Frame, app: &App, area: Rect) {
    let Some(finder) = app.finder() else { return };
    let width = area.width.saturating_sub(6).clamp(30, 110);
    let max_height = area.height.saturating_sub(4).clamp(3, 30);
    let height = u16::try_from(finder.matches.len())
        .unwrap_or(u16::MAX)
        .saturating_add(2) // the borders carry the title and the prompt
        .clamp(3, max_height);
    let rect = center(area, width, height);
    let inner_width = usize::from(width.saturating_sub(2));

    let highlight = Style::default()
        .fg(Color::Rgb(214, 160, 30))
        .add_modifier(Modifier::BOLD);
    let items: Vec<ListItem<'_>> = finder
        .matches
        .iter()
        .enumerate()
        .filter_map(|(index, m)| {
            let entry = finder.entry(index)?;
            // Context first, dimmed; the line text carries the highlights.
            let context = format!(" {} · {} · ", truncate(&entry.title, 24), entry.tab.title());
            let room = inner_width.saturating_sub(context.chars().count() + 1);
            let mut spans = vec![Span::styled(context, Style::default().fg(Color::DarkGray))];
            let mut segment = String::new();
            let mut segment_matched = None;
            for (offset, c) in entry.text.chars().take(room).enumerate() {
                let matched = m.positions.contains(&offset);
                if segment_matched.is_some_and(|was| was != matched) && !segment.is_empty() {
                    let style = if matched { Style::default() } else { highlight };
                    spans.push(Span::styled(std::mem::take(&mut segment), style));
                }
                segment_matched = Some(matched);
                segment.push(c);
            }
            if !segment.is_empty() {
                let style = if segment_matched == Some(true) {
                    highlight
                } else {
                    Style::default()
                };
                spans.push(Span::styled(segment, style));
            }
            Some(ListItem::new(Line::from(spans)))
        })
        .collect();

    let title = if finder.query.is_empty() {
        " find everywhere — type to search · esc closes ".to_owned()
    } else {
        format!(
            " find everywhere ({} matches) — ↑/↓ move · enter jump · esc ",
            finder.matches.len()
        )
    };
    let block = Block::default()
        .title(title)
        .title_alignment(Alignment::Center)
        .title_bottom(Line::from(format!(" \\ {}▌ ", finder.query)).left_aligned())
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Yellow));
    let list = List::new(items).block(block).highlight_style(
        Style::default()
            .bg(Color::Rgb(40, 44, 60))
            .add_modifier(Modifier::BOLD),
    );
    let mut state = ListState::default();
    if !finder.matches.is_empty() {
        state.select(Some(finder.selected));
    }
    frame.render_widget(Clear, rect);
    frame.render_stateful_widget(list, rect, &mut state);
}
