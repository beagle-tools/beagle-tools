//! Drawing. Reads [`App`] state; the only thing it writes back is layout
//! (viewport height and the width-dependent line wrap cache).

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Padding, Paragraph};
use ratatui::Frame;

use beagle_tui::widgets::{self, border_style, truncate, LIST_HIGHLIGHT};

use super::badges::{badges, comment_badges, tab_title};
use super::confirm::draw_confirm;
use super::doc::state_color;
use super::keys::HELP;
use super::{App, Focus, Tab};
use crate::store::Shelf;

/// The PR list's default width (`w` resizes it).
pub(super) const SIDEBAR_WIDTH: u16 = 38;
/// The Comments tab puts the agent's review beside the thread only when the
/// main pane has at least this many columns.
const COMMENTS_SPLIT_MIN: u16 = SIDEBAR_WIDTH * 2;

pub(super) fn draw(frame: &mut Frame<'_>, app: &mut App) {
    let [header, body, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(1),
    ])
    .areas(frame.area());
    let [side, main] = Layout::horizontal([
        Constraint::Length(app.side.width(body.width)),
        Constraint::Min(10),
    ])
    .areas(body);

    draw_header(frame, app, header);
    draw_items(frame, app, side);
    draw_content(frame, app, main);
    draw_footer(frame, app, footer);
    if app.help {
        widgets::draw_help(frame, HELP);
    }
    if let Some(confirm) = &app.confirm {
        draw_confirm(frame, confirm);
    }
}

fn draw_header(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let dim = Style::default().fg(Color::DarkGray);
    let mut spans = vec![Span::styled(
        " recap ",
        Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )];
    if app.shelf == Shelf::Archived {
        spans.push(Span::styled(
            " ARCHIVE ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
    }
    match app.recap.as_ref() {
        Some(r) => {
            let m = &r.manifest;
            spans.push(Span::styled(
                format!(" {}#{} ", m.repo, m.number),
                Style::default().add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled(
                r.state().label(),
                Style::default().fg(state_color(r.state())),
            ));
            if let Some(status) = app.selected_status() {
                spans.extend(badges(status, false));
            }
            if let Some(t) = &m.ticket {
                spans.push(Span::styled(format!("  ·  {t}"), dim));
            }
            spans.push(Span::styled(
                format!("   PR {}/{}", app.selected + 1, app.rows.len()),
                dim,
            ));
        }
        None => spans.push(Span::styled(
            format!(" no recaps yet in {}", app.store.recaps_dir().display()),
            dim,
        )),
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_items(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let width = usize::from(area.width.saturating_sub(4));
    let mut selected_entry = None;
    let items: Vec<ListItem<'static>> = super::list_entries(&app.rows)
        .into_iter()
        .enumerate()
        .map(|(i, entry)| match entry {
            super::ListEntry::Header(state, count) => {
                let label = format!("── {} · {count} ", state.heading());
                let fill = "─".repeat(width.saturating_sub(label.chars().count()) + 2);
                ListItem::new(Line::from(vec![
                    Span::styled(
                        label,
                        Style::default()
                            .fg(state_color(state))
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(fill, Style::default().fg(Color::DarkGray)),
                ]))
            }
            super::ListEntry::Row(index) => {
                if index == app.selected {
                    selected_entry = Some(i);
                }
                let r = &app.rows[index];
                let color = state_color(r.state);
                let repo = r.repo.rsplit('/').next().unwrap_or(&r.repo);
                let mut first = vec![
                    Span::styled("● ", Style::default().fg(color)),
                    Span::styled(
                        format!("{repo}#{}", r.number),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                ];
                if let Some(status) = app.statuses.get(&r.url) {
                    first.extend(badges(status, true));
                }
                if let Some(m) = app.comment_marks.get(&r.id) {
                    first.extend(comment_badges(*m));
                }
                ListItem::new(vec![
                    Line::from(first),
                    Line::from(Span::raw(format!(
                        "  {}",
                        truncate(&r.title, width.saturating_sub(2))
                    ))),
                ])
            }
        })
        .collect();
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style(app.focus == Focus::List))
                .title(match app.shelf {
                    Shelf::Active => format!(" PRs · {} ", app.rows.len()),
                    Shelf::Archived => format!(" Archived · {} ", app.rows.len()),
                }),
        )
        .highlight_style(
            Style::default()
                .bg(LIST_HIGHLIGHT)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▌");
    let mut state = ListState::default().with_selected(selected_entry);
    frame.render_stateful_widget(list, area, &mut state);
}

fn draw_content(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    // Comments: the shown thread, and beside it — about as wide — the
    // agent's review of it.
    let area = if app.tab == Tab::Comments && area.width > COMMENTS_SPLIT_MIN {
        let [thread, review] =
            Layout::horizontal([Constraint::Percentage(52), Constraint::Percentage(48)])
                .areas(area);
        draw_review_pane(frame, app, review);
        thread
    } else {
        area
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style(app.focus == Focus::Content))
        .padding(Padding::left(1))
        .title(tab_title(app));
    let inner = block.inner(area);
    app.view.viewport = inner.height;
    // Leave a column of breathing room on the right of wrapped prose.
    app.layout(inner.width.saturating_sub(1));
    app.view.scroll = app.view.scroll.min(app.view.max_scroll());
    // Showing a thread in the Comments tab is reading it.
    if app.tab == Tab::Comments {
        app.mark_current_read();
        app.layout(inner.width.saturating_sub(1));
    }

    let lines = if app.recap.is_none() {
        empty_state(app)
    } else {
        app.view.visible()
    };
    widgets::draw_doc(frame, area, block, &mut app.view, lines);

    // The line cursor and visual selection, only while the pane has focus.
    if app.focus == Focus::Content && app.recap.is_some() {
        widgets::paint_cursor(frame, area, inner, &app.view);
    }
}

/// The Comments tab's right-hand pane: the agent's review of the shown
/// thread, scrolled with `J`/`K`.
fn draw_review_pane(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    let width = area.width.saturating_sub(4);
    let (title, lines) = super::threads::review_pane(app, width);
    let height = area.height.saturating_sub(2);
    let max = u16::try_from(lines.len())
        .unwrap_or(u16::MAX)
        .saturating_sub(height);
    app.review_scroll = app.review_scroll.min(max);
    frame.render_widget(
        Paragraph::new(lines).scroll((app.review_scroll, 0)).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(if app.focus == Focus::Review {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default().fg(Color::Magenta)
                })
                .padding(Padding::horizontal(1))
                .title(title),
        ),
        area,
    );
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
            "  No PR recaps yet.",
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Line::default(),
        Line::styled("  Pull one in, then have an agent write it up:", dim),
        Line::styled(
            "    beagle recap new https://github.com/<owner>/<repo>/pull/<n>",
            Style::default().fg(Color::Cyan),
        ),
        Line::default(),
        Line::styled(
            format!("  Recaps live in {}", app.store.recaps_dir().display()),
            dim,
        ),
        Line::styled("  This screen updates live as files are written.", dim),
    ]);
    lines
}

fn draw_footer(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let line = if app.side.is_active() {
        Line::from(vec![
            Span::styled(
                format!(" {}", beagle_tui::pane_resize::HINT),
                Style::default().fg(Color::Cyan),
            ),
            Span::styled(
                app.status
                    .as_ref()
                    .map(|s| format!("   {s}"))
                    .unwrap_or_default(),
                Style::default().fg(Color::Yellow),
            ),
        ])
    } else if let Some(busy) = &app.busy {
        Line::styled(format!(" ⟳ {busy}…"), Style::default().fg(Color::Cyan))
    } else if let Some(status) = &app.status {
        Line::styled(format!(" {status}"), Style::default().fg(Color::Yellow))
    } else if let Some(err) = &app.comments_error {
        Line::styled(
            format!(" ⚠ comments sync failed — {err} (recaps/.comments-error.txt)"),
            Style::default().fg(Color::Red),
        )
    } else {
        let hints = match app.shelf {
            Shelf::Active => " j/k move · tab focus · 1/2/3 summary/diff/comments · o PR · i copy id · R ready · M merge · X close · a archive · A archive view · w resize · ? help · q quit",
            Shelf::Archived => " ARCHIVE · j/k move · 1/2 summary/diff · U restore · A back to PRs · w resize · ? help · q quit",
        };
        Line::styled(hints, Style::default().fg(Color::DarkGray))
    };
    frame.render_widget(Paragraph::new(line), area);
}
