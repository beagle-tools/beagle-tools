//! Drawing. Reads [`App`] state; the only thing it writes back is layout
//! (viewport height and the width-dependent line wrap cache).

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Padding, Paragraph};
use ratatui::Frame;

use beagle_tui::widgets::{self, border_style, truncate, LIST_HIGHLIGHT};

use super::doc::state_color;
use super::keys::HELP;
use super::{App, Focus};
use crate::model::ReportDate;

const SIDEBAR_WIDTH: u16 = 34;

pub(super) fn draw(frame: &mut Frame<'_>, app: &mut App) {
    let [header, body, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(1),
    ])
    .areas(frame.area());
    let [side, main] =
        Layout::horizontal([Constraint::Length(SIDEBAR_WIDTH), Constraint::Min(10)]).areas(body);

    draw_header(frame, app, header);
    draw_items(frame, app, side);
    draw_content(frame, app, main);
    draw_footer(frame, app, footer);
    if app.help {
        widgets::draw_help(frame, HELP);
    }
    if let Some(picker) = &app.picker {
        draw_picker(frame, app, picker);
    }
}

fn draw_picker(frame: &mut Frame<'_>, app: &App, picker: &super::Picker) {
    let current = app.current_date().map(|d| d.to_string());
    let items: Vec<ListItem<'static>> = picker
        .rows
        .iter()
        .map(|r| {
            let label = ReportDate::parse(&r.date).map_or_else(
                |_| r.date.clone(),
                |d| format!("{} {}", d.weekday(), r.date),
            );
            let mark = if Some(&r.date) == current.as_ref() {
                "● "
            } else {
                "  "
            };
            ListItem::new(vec![
                Line::from(vec![
                    Span::styled(mark, Style::default().fg(Color::Cyan)),
                    Span::styled(
                        format!("{label:<21}"),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("{:>2} tickets", r.tickets),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]),
                Line::from(Span::styled(
                    format!("  {}", truncate(r.title.as_deref().unwrap_or(""), 58)),
                    Style::default().fg(Color::Gray),
                )),
            ])
        })
        .collect();
    let area = frame.area();
    let height = u16::try_from(picker.rows.len() * 2 + 2)
        .unwrap_or(u16::MAX)
        .min(area.height.saturating_sub(2))
        .max(3.min(area.height));
    let rect = widgets::centered(area, 64, height);
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .title(" reports · j/k · enter open · esc "),
        )
        .highlight_style(Style::default().bg(LIST_HIGHLIGHT))
        .highlight_symbol("▌");
    let mut state = ListState::default().with_selected(Some(picker.selected));
    frame.render_widget(Clear, rect);
    frame.render_stateful_widget(list, rect, &mut state);
}

fn draw_header(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let dim = Style::default().fg(Color::DarkGray);
    let mut spans = vec![Span::styled(
        " standup ",
        Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )];
    match app.current_date() {
        Some(date) => {
            spans.push(Span::styled(
                format!(" {} {date}", date.weekday()),
                Style::default().add_modifier(Modifier::BOLD),
            ));
            if let Some(since) = app.report.as_ref().and_then(|r| r.manifest.since.as_ref()) {
                spans.push(Span::styled(format!("  since {since}"), dim));
            }
            // Name the neighbours by day, so "back to Monday's" is obvious.
            let older = app
                .dates
                .get(app.current + 1)
                .map_or_else(|| "—".to_owned(), |d| d.short());
            let newer = app
                .current
                .checked_sub(1)
                .and_then(|i| app.dates.get(i))
                .map_or_else(|| "—".to_owned(), |d| d.short());
            spans.push(Span::styled(
                format!(
                    "   report {}/{}  ·  [ {older}  ] {newer}  ·  d all reports",
                    app.current + 1,
                    app.dates.len()
                ),
                dim,
            ));
        }
        None => spans.push(Span::styled(
            format!(" no reports yet in {}", app.store.reports_dir().display()),
            dim,
        )),
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_items(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let width = usize::from(area.width.saturating_sub(4));
    let mut items = vec![ListItem::new(vec![
        Line::from(Span::styled(
            "★ Overview",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "  the big picture",
            Style::default().fg(Color::DarkGray),
        )),
    ])];
    if let Some(report) = &app.report {
        for t in &report.tickets {
            let color = state_color(t.state_kind());
            let state = t.manifest.state.as_deref().unwrap_or("—");
            items.push(ListItem::new(vec![
                Line::from(vec![
                    Span::styled("● ", Style::default().fg(color)),
                    Span::styled(
                        format!("{} ", t.id),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(state.to_owned(), Style::default().fg(color)),
                ]),
                Line::from(Span::raw(format!(
                    "  {}",
                    truncate(&t.manifest.title, width.saturating_sub(2))
                ))),
            ]));
        }
    }
    let count = app.report.as_ref().map_or(0, |r| r.tickets.len());
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style(app.focus == Focus::List))
                .title(format!(" Items · {count} tickets ")),
        )
        .highlight_style(
            Style::default()
                .bg(LIST_HIGHLIGHT)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▌");
    let mut state = ListState::default().with_selected(Some(app.selected));
    frame.render_stateful_widget(list, area, &mut state);
}

fn draw_content(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    let title = match (app.selected_ticket(), app.report.is_some()) {
        (Some(t), _) => format!(" {} ", t.id),
        (None, true) => " Overview ".to_owned(),
        (None, false) => " standup ".to_owned(),
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style(app.focus == Focus::Content))
        .padding(Padding::left(1))
        .title(title);
    let inner = block.inner(area);
    app.view.viewport = inner.height;
    // Leave a column of breathing room on the right of wrapped prose.
    app.view.layout(inner.width.saturating_sub(1));
    app.view.scroll = app.view.scroll.min(app.view.max_scroll());

    let lines = if app.report.is_none() {
        empty_state(app)
    } else {
        app.view.visible()
    };
    widgets::draw_doc(frame, area, block, &mut app.view, lines);

    // The line cursor and visual selection, only while the write-up has focus.
    if app.focus == Focus::Content && app.report.is_some() {
        widgets::paint_cursor(frame, area, inner, &app.view);
    }
}

fn empty_state(app: &App) -> Vec<Line<'static>> {
    let dim = Style::default().fg(Color::DarkGray);
    let mut lines = vec![Line::default()];
    if let Some(e) = &app.load_error {
        lines.push(Line::styled(
            format!("  {e}"),
            Style::default().fg(Color::Red),
        ));
        return lines;
    }
    lines.extend([
        Line::styled(
            "  No standup reports yet.",
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Line::default(),
        Line::styled(
            "  Ask an agent to write today's standup, or scaffold one yourself:",
            dim,
        ),
        Line::styled("    beagle standup new", Style::default().fg(Color::Cyan)),
        Line::default(),
        Line::styled(
            format!("  Reports live in {}", app.store.reports_dir().display()),
            dim,
        ),
        Line::styled("  This screen updates live as files are written.", dim),
    ]);
    lines
}

fn draw_footer(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let line = match &app.status {
        Some(status) => Line::styled(format!(" {status}"), Style::default().fg(Color::Yellow)),
        None => Line::styled(
            " j/k move · tab focus · v select · y copy · i id · o ticket · p PRs · d reports · [ ] older/newer · ? help · q quit",
            Style::default().fg(Color::DarkGray),
        ),
    };
    frame.render_widget(Paragraph::new(line), area);
}
