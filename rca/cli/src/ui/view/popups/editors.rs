//! The small editing popups: the `t` status picker, `#` tags editor,
//! `P`/`I` attach prompt, and `D` delete confirmation.

use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::ui::App;

use super::super::style::{center, truncate};

/// The `t` status picker: the five lifecycle stages with their glyphs and
/// a one-line hint each, the incident's current stage marked. Enter
/// applies, esc closes.
pub(in crate::ui::view) fn draw_status_picker(frame: &mut Frame, app: &App, area: Rect) {
    use crate::model::Status;

    let Some(picker) = app.status_picker() else {
        return;
    };
    let width = area.width.saturating_sub(6).clamp(40, 74);
    let height = u16::try_from(Status::ALL.len())
        .unwrap_or(u16::MAX)
        .saturating_add(2)
        .min(area.height.saturating_sub(2));
    let rect = center(area, width, height);

    let items: Vec<ListItem<'_>> = Status::ALL
        .iter()
        .map(|&status| {
            let (symbol, symbol_style) = crate::ui::view::style::status_symbol(status, 0);
            let hint = match status {
                Status::Investigating => "actively debugging",
                Status::Review => "write-up done; fix PR out for review",
                Status::Agent => "handed off to an automated agent",
                Status::FinalReview => "fix merged — work the checklist, V signs off",
                Status::Finished => "verified and closed",
            };
            let marker = if status == picker.current {
                " (current)"
            } else {
                ""
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!(" {symbol} "), symbol_style),
                Span::raw(format!("{:<13}", status.as_str())),
                Span::styled(
                    format!("{hint}{marker}"),
                    Style::default().fg(Color::DarkGray),
                ),
            ]))
        })
        .collect();

    let block = Block::default()
        .title(format!(
            " status — {} ",
            truncate(&picker.title, usize::from(width).saturating_sub(12))
        ))
        .title_alignment(Alignment::Center)
        .title_bottom(Line::from(" enter apply · j/k move · esc cancel ").centered())
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Yellow));
    let list = List::new(items).block(block).highlight_style(
        Style::default()
            .bg(Color::Rgb(40, 44, 60))
            .add_modifier(Modifier::BOLD),
    );
    let mut state = ListState::default();
    state.select(Some(picker.selected));
    frame.render_widget(Clear, rect);
    frame.render_stateful_widget(list, rect, &mut state);
}

/// The `#` tags editor: current tags as rows, a trailing `+ add tag` row,
/// and an inline input while typing. Adds/deletes write straight to the
/// manifest, so what you see is what's on disk.
pub(in crate::ui::view) fn draw_tags_editor(frame: &mut Frame, app: &App, area: Rect) {
    let Some(editor) = app.tags_editor() else {
        return;
    };
    let width = area.width.saturating_sub(6).clamp(36, 64);
    let inner = usize::from(width.saturating_sub(6));

    let mut items: Vec<ListItem<'_>> = editor
        .tags
        .iter()
        .map(|tag| {
            let special = tag == crate::model::SKIP_FINAL_REVIEW_TAG;
            let mut spans = vec![Span::raw(format!(" {}", truncate(tag, inner)))];
            if special {
                spans.push(Span::styled(
                    "  (merged PRs → finished)",
                    Style::default().fg(Color::Yellow),
                ));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();
    items.push(ListItem::new(Line::styled(
        match &editor.typing {
            Some(buffer) => format!(" + {buffer}▌"),
            None => " + add tag".to_owned(),
        },
        Style::default().fg(Color::DarkGray),
    )));

    let height = u16::try_from(items.len())
        .unwrap_or(u16::MAX)
        .saturating_add(2)
        .min(area.height.saturating_sub(2));
    let rect = center(area, width, height);

    let block = Block::default()
        .title(format!(
            " tags — {} ",
            truncate(&editor.title, usize::from(width).saturating_sub(10))
        ))
        .title_alignment(Alignment::Center)
        .title_bottom(
            Line::from(if editor.typing.is_some() {
                " type tag · enter add · esc back "
            } else {
                " a add · d delete · j/k move · esc close "
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
    state.select(Some(editor.selected));
    frame.render_widget(Clear, rect);
    frame.render_stateful_widget(list, rect, &mut state);
}

/// The `P`/`I` attach prompt: an input box for a PR URL or a ticket
/// reference to attach to the selected incident.
pub(in crate::ui::view) fn draw_attach_prompt(frame: &mut Frame, app: &App, area: Rect) {
    let Some(prompt) = app.attach_prompt() else {
        return;
    };
    let width = area.width.saturating_sub(6).clamp(40, 96);
    let inner = usize::from(width.saturating_sub(4));

    let lines = vec![
        Line::from(""),
        Line::styled(
            truncate(&prompt.title, inner),
            Style::default().fg(Color::DarkGray),
        )
        .centered(),
        Line::from(""),
        Line::from(vec![
            Span::styled(prompt.kind.label(), Style::default().fg(Color::Yellow)),
            Span::raw(format!("{}▌", prompt.input)),
        ]),
    ];
    let height = u16::try_from(lines.len())
        .unwrap_or(u16::MAX)
        .saturating_add(2)
        .min(area.height.saturating_sub(2));
    let rect = center(area, width, height);

    let block = Block::default()
        .title(prompt.kind.title())
        .title_alignment(Alignment::Center)
        .title_bottom(Line::from(" enter attach · esc cancel ").centered())
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Yellow));
    frame.render_widget(Clear, rect);
    frame.render_widget(Paragraph::new(lines).block(block), rect);
}

/// The `D` delete confirmation: names the incident (title + slug) and
/// waits for an explicit `y` or `n`. Red border — this one is destructive.
pub(in crate::ui::view) fn draw_confirm_delete(frame: &mut Frame, app: &App, area: Rect) {
    let Some(confirm) = app.confirm_delete() else {
        return;
    };
    let width = area.width.saturating_sub(6).clamp(30, 72);
    let inner = usize::from(width.saturating_sub(4));

    let lines = vec![
        Line::from(""),
        Line::styled(
            truncate(&confirm.title, inner),
            Style::default().add_modifier(Modifier::BOLD),
        )
        .centered(),
        Line::styled(
            truncate(&confirm.id.to_string(), inner),
            Style::default().fg(Color::DarkGray),
        )
        .centered(),
        Line::from(""),
        Line::styled(
            "permanently deletes the workspace directory and all its files",
            Style::default().fg(Color::Red),
        )
        .centered(),
    ];
    let height = u16::try_from(lines.len())
        .unwrap_or(u16::MAX)
        .saturating_add(2)
        .min(area.height.saturating_sub(2));
    let rect = center(area, width, height);

    let block = Block::default()
        .title(" delete this incident? ")
        .title_alignment(Alignment::Center)
        .title_bottom(Line::from(" y delete · n / esc cancel ").centered())
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Red));
    frame.render_widget(Clear, rect);
    frame.render_widget(Paragraph::new(lines).block(block), rect);
}
