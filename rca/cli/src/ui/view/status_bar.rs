//! The bottom status bar: the filter prompt, in-content search state, or
//! the transient status message plus key hints.

use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::ui::{App, Focus};

/// The filter-mode prompt: active facet chips, the free-text query (with a
/// cursor while typing), and the keys available in the current sub-mode.
pub(super) fn filter_prompt_line(app: &App) -> Line<'static> {
    let mut spans = vec![Span::styled(
        "  filter: ",
        Style::default().fg(Color::Yellow),
    )];
    let facets = app.facet_label();
    if !facets.is_empty() {
        spans.push(Span::styled(
            format!("{facets} "),
            Style::default().fg(Color::LightBlue),
        ));
    }
    spans.push(Span::raw(app.filter().to_owned()));
    if app.filter_typing() {
        spans.push(Span::styled("▌", Style::default().fg(Color::Yellow)));
        spans.push(Span::styled(
            "   type keywords · esc back to facets",
            Style::default().fg(Color::DarkGray),
        ));
    } else {
        spans.push(Span::styled(
            "   i/r/a/v/f status · c/h/m/l severity · p has-PR · s security · / type · esc clear",
            Style::default().fg(Color::DarkGray),
        ));
    }
    Line::from(spans)
}

/// The in-content search's hint line: live query, match position, or the
/// no-matches report.
pub(super) fn search_status_line(search: &crate::ui::search::ContentSearch) -> Line<'static> {
    if search.typing {
        Line::from(vec![
            Span::styled("  search: ", Style::default().fg(Color::Yellow)),
            Span::raw(search.query.clone()),
            Span::styled("▌", Style::default().fg(Color::Yellow)),
            Span::styled(
                "   enter commit · esc cancel",
                Style::default().fg(Color::DarkGray),
            ),
        ])
    } else if search.hits.is_empty() {
        Line::from(Span::styled(
            format!(
                "  no matches for \"{}\" anywhere in this incident — esc clears",
                search.query
            ),
            Style::default().fg(Color::Yellow),
        ))
    } else {
        let place = search
            .hits
            .get(search.current)
            .map_or("", |hit| hit.tab.title());
        Line::from(vec![
            Span::styled(
                format!(
                    "  match {}/{} for \"{}\" · {place}",
                    search.current + 1,
                    search.hits.len(),
                    search.query,
                ),
                Style::default().fg(Color::Yellow),
            ),
            Span::styled(
                "   n/N next/prev (across tabs) · esc clear",
                Style::default().fg(Color::DarkGray),
            ),
        ])
    }
}

pub(super) fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    if app.search_active() {
        frame.render_widget(Paragraph::new(filter_prompt_line(app)), area);
        return;
    }
    // A status message gets the whole line — sharing it with the key hints
    // truncated messages into uselessness. It disappears on the next
    // keypress, so the hints are only ever hidden for one beat.
    if let Some(message) = app.status_line() {
        let (symbol, color) = if message.contains("failed") {
            ("  ✗ ", Color::LightRed)
        } else {
            ("  ✓ ", Color::LightGreen)
        };
        let line = Line::from(vec![
            Span::styled(symbol, Style::default().fg(color)),
            Span::styled(message.to_owned(), Style::default().fg(color)),
        ]);
        frame.render_widget(Paragraph::new(line), area);
        return;
    }
    // In-content search owns the hint line while it exists; a transient
    // status message (handled above) still gets its one beat.
    if let Some(search) = app.content_search() {
        frame.render_widget(Paragraph::new(search_status_line(search)), area);
        return;
    }
    let mut spans = vec![Span::styled(
        match app.focus() {
            Focus::List => {
                "  j/k select · enter open · ←/→ tabs · f filter · / search · T toolbox · R related · c copy · r reload · ? help · Q quit"
            }
            Focus::Content => {
                "  j/k line · v select · y yank · ←/→ tabs · / search · F follow · s sidebar · c copy · b back · ? help · Q quit"
            }
        },
        Style::default().fg(Color::DarkGray),
    )];
    if app.follow() {
        spans.push(Span::styled(
            "  ·  following (esc exits)",
            Style::default().fg(Color::LightYellow),
        ));
    }
    let problems = app.broken().len() + app.warnings().len();
    if problems > 0 {
        spans.push(Span::styled(
            format!("  ·  ⚠ {problems} problem(s) — ! for details"),
            Style::default().fg(Color::Yellow),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}
