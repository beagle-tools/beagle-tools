//! Rendering: btop-flavored header + three panes (ACTIVE, RECENT, EVENTS)
//! and a centered help overlay.

use agent_core::types::RunState;
use chrono::{Local, Utc};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Cell, Clear, Paragraph, Row, Table, TableState, Wrap};
use ratatui::Frame;

use crate::app::{App, EMPTY_HINT};
use crate::format::{elapsed_between, payload_snippet, truncate_chars};

const KEY_HINTS: &str =
    "q:quit  \u{2191}\u{2193}:select  enter:attach/logs  d:jobs  f:filter  x:kill  ?:help";
const EVENT_SNIPPET_CHARS: usize = 120;
const RUN_ID_WIDTH: u16 = 30;

pub fn draw(frame: &mut Frame<'_>, app: &App) {
    // The log view first. It used to be checked AFTER the jobs screens,
    // which returned early — so enter on a step set the log and nothing
    // ever drew it. A log is always the screen on top, whichever screen
    // opened it.
    if let Some(view) = &app.log_view {
        crate::logs::draw(frame, view, app.status_message());
        if app.show_help {
            draw_help(frame);
        }
        return;
    }
    if app.show_dag {
        crate::dag::draw(frame, app);
        if app.show_run_detail {
            crate::detail::draw_overlay(frame, app);
        }
        if app.show_help {
            draw_help(frame);
        }
        return;
    }
    let [header, body, events] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Percentage(72),
        Constraint::Min(3),
    ])
    .areas(frame.area());
    // Tables left, the selected run's detail right — the same shape as the
    // DAG page, so the preview lives in the same place on both screens.
    let [tables, preview] =
        Layout::horizontal([Constraint::Percentage(62), Constraint::Percentage(38)]).areas(body);
    let [active, recent] =
        Layout::vertical([Constraint::Percentage(45), Constraint::Percentage(55)]).areas(tables);

    draw_header(frame, app, header);
    draw_active(frame, app, active);
    draw_recent(frame, app, recent);
    draw_preview(frame, app, preview);
    draw_events(frame, app, events);
    if app.show_run_detail {
        crate::detail::draw_overlay(frame, app);
    }
    if app.show_help {
        draw_help(frame);
    }
}

fn draw_header(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let running = app
        .active
        .iter()
        .filter(|r| r.state == RunState::Running)
        .count();
    let pending = app.active.len() - running;
    let clock = Local::now().format("%H:%M:%S");

    let mut spans = vec![
        Span::styled(
            " agenttop ",
            Style::new()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!("  agents:{}", app.agent_count)),
        Span::styled(
            format!("  running:{running}"),
            Style::new().fg(Color::Green),
        ),
        Span::styled(
            format!("  pending:{pending}"),
            Style::new().fg(Color::Yellow),
        ),
        Span::raw(format!("  {clock}  ")),
    ];
    // A retention problem outranks the key hints: write-ups are piling up
    // because the daemon cannot reach GitHub, and nothing else would say so.
    if let Some(problem) = &app.retention_problem {
        spans.push(Span::styled(
            format!(" ⚠ {problem} "),
            Style::new().fg(Color::Red).add_modifier(Modifier::BOLD),
        ));
    } else if let Some(status) = app.status_message() {
        spans.push(Span::styled(
            format!(" {status} "),
            Style::new().fg(Color::Magenta).add_modifier(Modifier::BOLD),
        ));
    } else {
        spans.push(Span::styled(
            KEY_HINTS,
            Style::new().add_modifier(Modifier::DIM),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn pane_block(title: &str) -> Block<'static> {
    Block::bordered().title(Span::styled(
        format!(" {title} "),
        Style::new().add_modifier(Modifier::BOLD),
    ))
}

fn header_row<const N: usize>(titles: [&str; N]) -> Row<'_> {
    Row::new(titles).style(Style::new().add_modifier(Modifier::BOLD | Modifier::DIM))
}

fn draw_active(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let block = pane_block("ACTIVE");
    if let Some(error) = &app.store_error {
        let text = Paragraph::new(error.as_str())
            .style(Style::new().fg(Color::Red))
            .wrap(Wrap { trim: true })
            .block(block);
        frame.render_widget(text, area);
        return;
    }
    if app.is_empty() {
        let text = Paragraph::new(EMPTY_HINT)
            .style(Style::new().add_modifier(Modifier::DIM))
            .wrap(Wrap { trim: true })
            .block(block);
        frame.render_widget(text, area);
        return;
    }

    let now = Utc::now();
    let rows = app.active.iter().map(|run| {
        let started = run.started_at.unwrap_or(run.created_at);
        let activity = app.last_activity.get(&run.id).cloned().unwrap_or_default();
        Row::new(vec![
            Cell::from(run.id.clone()),
            Cell::from(format!("{}/{}", run.project, run.agent)),
            Cell::from(run.state.as_str()).style(state_style(run.state)),
            Cell::from(run.trigger.as_str()),
            Cell::from(elapsed_between(started, now)),
            Cell::from(activity).style(Style::new().add_modifier(Modifier::DIM)),
        ])
    });
    let table = Table::new(
        rows,
        [
            Constraint::Length(RUN_ID_WIDTH),
            Constraint::Length(22),
            Constraint::Length(9),
            Constraint::Length(7),
            Constraint::Length(8),
            Constraint::Min(10),
        ],
    )
    .header(header_row([
        "RUN ID",
        "PROJECT/AGENT",
        "STATE",
        "TRIGGER",
        "ELAPSED",
        "LAST ACTIVITY",
    ]))
    .row_highlight_style(Style::new().add_modifier(Modifier::REVERSED))
    .block(block);

    let mut state = TableState::default();
    if app.selected < app.active.len() {
        state.select(Some(app.selected));
    }
    frame.render_stateful_widget(table, area, &mut state);
}

fn draw_recent(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let block = pane_block(&format!("RECENT{}", app.recent_filter_label()));
    let now = Utc::now();
    let rows = app.recent.iter().map(|run| {
        let duration = run.finished_at.map_or_else(
            || "-".to_owned(),
            |end| elapsed_between(run.started_at.unwrap_or(run.created_at), end),
        );
        let exit = run
            .exit_code
            .map_or_else(|| "-".to_owned(), |code| code.to_string());
        let age = elapsed_between(run.finished_at.unwrap_or(run.created_at), now);
        Row::new(vec![
            Cell::from(run.id.clone()),
            Cell::from(format!("{}/{}", run.project, run.agent)),
            Cell::from(run.state.as_str()).style(state_style(run.state)),
            Cell::from(duration),
            Cell::from(crate::format::cost(run.cost_usd)),
            Cell::from(exit),
            Cell::from(age),
        ])
    });
    let table = Table::new(
        rows,
        [
            Constraint::Length(RUN_ID_WIDTH),
            Constraint::Length(22),
            Constraint::Length(9),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(4),
            Constraint::Min(6),
        ],
    )
    .header(header_row([
        "RUN ID",
        "PROJECT/AGENT",
        "STATE",
        "DURATION",
        "COST",
        "EXIT",
        "AGE",
    ]))
    .row_highlight_style(Style::new().add_modifier(Modifier::REVERSED))
    .block(block);

    let mut state = TableState::default();
    if app.selected >= app.active.len() && app.selected < app.row_count() {
        state.select(Some(app.selected - app.active.len()));
    }
    frame.render_stateful_widget(table, area, &mut state);
}

/// The selected run's detail, beside the tables. Same content as the DAG
/// page's RUN pane, so there is one place to learn what these fields mean.
fn draw_preview(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let Some(run) = app.selected_run() else {
        frame.render_widget(
            Paragraph::new("no run selected")
                .style(Style::new().add_modifier(Modifier::DIM))
                .block(pane_block("RUN")),
            area,
        );
        return;
    };
    let visible = usize::from(area.height.saturating_sub(2));
    let mut lines = crate::detail::run_detail(run);
    lines.truncate(visible);
    frame.render_widget(
        Paragraph::new(lines).block(pane_block(&format!("RUN · {} · O:open  m:more", run.agent))),
        area,
    );
}

fn draw_events(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let visible = usize::from(area.height.saturating_sub(2));
    let start = app.events.len().saturating_sub(visible);
    let lines: Vec<Line<'_>> = app.events[start..]
        .iter()
        .map(|event| {
            let ts = event.ts.with_timezone(&Local).format("%H:%M:%S");
            Line::from(vec![
                Span::styled(format!("{ts} "), Style::new().add_modifier(Modifier::DIM)),
                Span::styled(
                    format!("{} ", truncate_chars(&event.run_id, 28)),
                    Style::new().fg(Color::Cyan),
                ),
                Span::styled(
                    format!("{} ", event.kind),
                    Style::new().add_modifier(Modifier::BOLD),
                ),
                Span::raw(payload_snippet(
                    event.payload.as_deref(),
                    EVENT_SNIPPET_CHARS,
                )),
            ])
        })
        .collect();
    frame.render_widget(Paragraph::new(lines).block(pane_block("EVENTS")), area);
}

fn draw_help(frame: &mut Frame<'_>) {
    let lines = [
        "q / ctrl-c   quit (q first closes an open overlay)",
        "j / \u{2193}        select next row / scroll logs down",
        "k / \u{2191}        select previous row / scroll logs up",
        "enter        attach to a running run (tmux; detach: ctrl-b d),",
        "             or open the event log of a finished run",
        "l            view the selected run's event log",
        "h / l / \u{2190}\u{2192}    pan the log view sideways (unwraps; h back to 0 rewraps)",
        "y            copy: the whole log (log view) / run id (dashboard)",
        "d            DAG page: projects \u{2192} agents, plus run lineage flows",
        "             (there: j/k select an agent, e edits its manifest)",
        "f            cycle RECENT filter: all \u{2192} succeeded \u{2192} failed",
        "x            kill the selected running run",
        "r            force refresh",
        "esc / q / ?  close this help or the log view",
        "",
        "selection is one list: ACTIVE rows first, then RECENT.",
    ];
    let height = u16::try_from(lines.len())
        .unwrap_or(u16::MAX)
        .saturating_add(2);
    let area = centered_rect(60, height, frame.area());
    let text: Vec<Line<'_>> = lines.iter().map(|line| Line::from(*line)).collect();
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(text)
            .block(pane_block("HELP"))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    Rect {
        x: area.x + (area.width - w) / 2,
        y: area.y + (area.height - h) / 2,
        width: w,
        height: h,
    }
}

fn state_style(state: RunState) -> Style {
    match state {
        RunState::Running => Style::new().fg(Color::Green),
        RunState::Pending => Style::new().fg(Color::Yellow),
        RunState::Succeeded => Style::new().fg(Color::Blue).add_modifier(Modifier::DIM),
        RunState::Failed => Style::new().fg(Color::Red),
        RunState::TimedOut => Style::new().fg(Color::Magenta),
        RunState::Killed => Style::new().fg(Color::DarkGray),
    }
}
