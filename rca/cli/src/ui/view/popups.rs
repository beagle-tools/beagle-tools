//! Popup rendering: the `o` link picker, the `R` related-incidents picker,
//! the `S` settings overlay, the `T` toolbox overlay, and the `!` errors
//! overlay here; the `\` finder, the editing popups, and the `?` help sheet
//! in child modules. All render centered over the main layout with a
//! `Clear` underneath.

mod editors;
mod finder;
mod help;

pub(super) use editors::{
    draw_attach_prompt, draw_confirm_delete, draw_status_picker, draw_tags_editor,
};
pub(super) use finder::draw_finder;
pub(super) use help::draw_help;

use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap,
};
use ratatui::Frame;

use crate::ui::App;

use super::style::{center, pr_color, truncate};

/// The `o` popup: attached PRs (with live state glyphs when known) plus
/// URLs found on the current tab. Enter opens in the browser.
pub(super) fn draw_links(frame: &mut Frame, app: &App, area: Rect) {
    let Some(popup) = app.links() else { return };
    let width = area.width.saturating_sub(6).clamp(20, 100);
    let height = u16::try_from(popup.items.len())
        .unwrap_or(u16::MAX)
        .saturating_add(2)
        .min(area.height.saturating_sub(2));
    let rect = center(area, width, height);

    let items: Vec<ListItem<'_>> = popup
        .items
        .iter()
        .map(|url| {
            let mut spans = Vec::new();
            if let Some(state) = app.pr_state(url) {
                spans.push(Span::styled(
                    format!(" {} {} ", state.glyph(), state.label()),
                    Style::default().fg(pr_color(state)),
                ));
            } else {
                spans.push(Span::raw(" "));
            }
            spans.push(Span::raw(truncate(
                url,
                usize::from(width).saturating_sub(14),
            )));
            ListItem::new(Line::from(spans))
        })
        .collect();
    let block = Block::default()
        .title(" links — enter open · j/k move · esc close ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Yellow));
    let list = List::new(items).block(block).highlight_style(
        Style::default()
            .bg(Color::Rgb(40, 44, 60))
            .add_modifier(Modifier::BOLD),
    );
    let mut state = ListState::default();
    state.select(Some(popup.selected));
    frame.render_widget(Clear, rect);
    frame.render_stateful_widget(list, rect, &mut state);
}

/// The `R` popup: past incidents sharing systems/tags with the selected
/// one, best match first. Enter jumps to the workspace.
pub(super) fn draw_related(frame: &mut Frame, app: &App, area: Rect) {
    let Some(popup) = app.related() else { return };
    let width = area.width.saturating_sub(6).clamp(30, 110);
    let height = u16::try_from(popup.items.len())
        .unwrap_or(u16::MAX)
        .saturating_add(2)
        .min(area.height.saturating_sub(2));
    let rect = center(area, width, height);

    let items: Vec<ListItem<'_>> = popup
        .items
        .iter()
        .map(|item| {
            let (badge, badge_style) = super::style::severity_badge(item.severity);
            let (symbol, symbol_style) = super::style::status_symbol(item.status, 0);
            // Leave room for badge/status/shared so the title never pushes
            // the "why it ranked" note off screen.
            let title_room = usize::from(width).saturating_sub(item.shared.len() + 24);
            ListItem::new(Line::from(vec![
                Span::styled(format!(" {badge} "), badge_style),
                Span::styled(format!(" {symbol} "), symbol_style),
                Span::raw(truncate(&item.title, title_room.max(12))),
                Span::styled(
                    format!("  ({})", item.shared),
                    Style::default().fg(Color::DarkGray),
                ),
            ]))
        })
        .collect();
    let block = Block::default()
        .title(" related incidents — enter jump · j/k move · esc close ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Yellow));
    let list = List::new(items).block(block).highlight_style(
        Style::default()
            .bg(Color::Rgb(40, 44, 60))
            .add_modifier(Modifier::BOLD),
    );
    let mut state = ListState::default();
    state.select(Some(popup.selected));
    frame.render_widget(Clear, rect);
    frame.render_stateful_widget(list, rect, &mut state);
}

/// The `S` settings overlay: config fields with their current values, a
/// toggle/edit affordance per row, and the file path in the title. Every
/// change is written straight to the config file.
pub(super) fn draw_settings(frame: &mut Frame, app: &App, area: Rect) {
    use crate::ui::settings::Field;

    let Some(overlay) = app.settings() else {
        return;
    };
    let width = area.width.saturating_sub(8).clamp(44, 90);
    let height = u16::try_from(Field::ALL.len())
        .unwrap_or(u16::MAX)
        .saturating_add(4)
        .min(area.height.saturating_sub(2));
    let rect = center(area, width, height);

    let items: Vec<ListItem<'_>> = Field::ALL
        .iter()
        .map(|field| {
            let editing_here =
                overlay.editing.is_some() && Field::ALL.get(overlay.selected) == Some(field);
            // Set values pop green; unset ones keep the normal foreground
            // (dark gray was unreadable) — the parentheses already say
            // "this is a default".
            let value_style = if overlay.is_set(*field) || editing_here {
                Style::default().fg(Color::LightGreen)
            } else {
                Style::default()
            };
            let value = if editing_here {
                format!("{}▌", overlay.editing.clone().unwrap_or_default())
            } else {
                overlay.value_of(*field)
            };
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!(" {:<8}", field.key()),
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled(format!("{value:<28} "), value_style),
                Span::styled(
                    field.describe().to_owned(),
                    Style::default().fg(Color::DarkGray),
                ),
            ]))
        })
        .collect();

    let block = Block::default()
        .title(format!(" Settings — {} ", overlay.path.display()))
        .title_alignment(Alignment::Center)
        .title_bottom(
            Line::from(if overlay.editing.is_some() {
                " type value · enter save · esc cancel "
            } else {
                " enter/space edit or toggle · j/k move · esc close "
            })
            .centered(),
        )
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Yellow));
    let list = List::new(items).block(block).highlight_style(
        Style::default()
            .bg(Color::Rgb(40, 44, 60))
            .add_modifier(Modifier::BOLD),
    );
    let mut state = ListState::default();
    state.select(Some(overlay.selected));
    frame.render_widget(Clear, rect);
    frame.render_stateful_widget(list, rect, &mut state);

    if let Some(note) = &overlay.note {
        let note_rect = Rect {
            y: rect.y + rect.height.saturating_sub(2),
            height: 1,
            x: rect.x + 2,
            width: rect.width.saturating_sub(4),
        };
        frame.render_widget(
            Paragraph::new(Span::styled(
                note.clone(),
                Style::default().fg(Color::LightGreen),
            )),
            note_rect,
        );
    }
}

/// The toolbox overlay: root `toolbox.md` plus relevant `systems/*.md`,
/// pre-rendered by the app when opened. Scrollable; geometry is fed back
/// for clamping, like the content pane.
pub(super) fn draw_toolbox(frame: &mut Frame, app: &mut App, area: Rect) {
    let Some(text) = app.toolbox() else { return };
    let width = area.width.saturating_sub(6).clamp(20, 96);
    let height = area.height.saturating_sub(2).max(5);
    let popup = center(area, width, height);

    let block = Block::default()
        .title(" Toolbox — j/k scroll · T/esc close ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Yellow));
    let inner = block.inner(popup);

    let paragraph = Paragraph::new(text.clone()).wrap(Wrap { trim: false });
    let content_lines = u16::try_from(paragraph.line_count(inner.width)).unwrap_or(u16::MAX);
    app.toolbox_viewport = (content_lines, inner.height);
    let scroll = app
        .toolbox_scroll()
        .min(content_lines.saturating_sub(inner.height));

    frame.render_widget(Clear, popup);
    frame.render_widget(paragraph.block(block).scroll((scroll, 0)), popup);
}

/// The `!` errors overlay: every broken workspace with its full reason,
/// then any load warnings. Scrollable; geometry is fed back for clamping,
/// like the toolbox and content panes.
pub(super) fn draw_errors(frame: &mut Frame, app: &mut App, area: Rect) {
    if !app.errors_visible() {
        return;
    }
    let width = area.width.saturating_sub(6).clamp(30, 96);
    let height = area.height.saturating_sub(2).max(5);
    let popup = center(area, width, height);

    let mut lines: Vec<Line<'static>> = Vec::new();
    if !app.broken().is_empty() {
        lines.push(Line::styled(
            "Unloadable workspaces",
            Style::default()
                .fg(Color::LightRed)
                .add_modifier(Modifier::BOLD),
        ));
        for broken in app.broken() {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled(
                    " ⚠ ",
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Red)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" {}", broken.dir_name),
                    Style::default()
                        .fg(Color::LightRed)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
            lines.push(Line::styled(
                broken.reason.clone(),
                Style::default().fg(Color::Red),
            ));
        }
    }
    if !app.warnings().is_empty() {
        if !app.broken().is_empty() {
            lines.push(Line::from(""));
        }
        lines.push(Line::styled(
            "Warnings",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
        for warning in app.warnings() {
            lines.push(Line::styled(
                format!("• {}", warning.0),
                Style::default().fg(Color::Yellow),
            ));
        }
    }

    let block = Block::default()
        .title(" load errors — j/k scroll · c copy · !/esc close ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Red));
    let inner = block.inner(popup);

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    let content_lines = u16::try_from(paragraph.line_count(inner.width)).unwrap_or(u16::MAX);
    app.errors_viewport = (content_lines, inner.height);
    let scroll = app
        .errors_scroll()
        .min(content_lines.saturating_sub(inner.height));

    frame.render_widget(Clear, popup);
    frame.render_widget(paragraph.block(block).scroll((scroll, 0)), popup);
}
