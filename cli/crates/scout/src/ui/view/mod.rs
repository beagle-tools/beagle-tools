//! Drawing: a pure function of [`App`], apart from clamping the scroll to
//! what the content's real height allows.

use std::time::{Duration, SystemTime};

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap,
};
use ratatui::Frame;

use crate::model::{ProjectStatus, TaskStatus};
use crate::store::{Project, Task};

use super::keys::HELP;
use super::{App, Focus, Screen, Tab, WrapLayout};

/// Beagle's spinner: the "an agent is on it right now" signal.
const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// A working task whose files have been quiet this long gets a yellow
/// "quiet" — a crashed agent and a busy one look the same otherwise.
const QUIET_AFTER: Duration = Duration::from_mins(10);

const WORKING: Color = Color::Indexed(208); // orange; the 16-colour palette has none

fn dim() -> Style {
    Style::default().fg(Color::DarkGray)
}

pub(super) fn draw(frame: &mut Frame<'_>, app: &mut App) {
    let [main, bar] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(frame.area());
    let sidebar = (main.width / 3).clamp(28, 48);
    let [side, content] =
        Layout::horizontal([Constraint::Length(sidebar), Constraint::Min(1)]).areas(main);
    match app.screen {
        Screen::Projects => {
            draw_project_list(frame, app, side);
            draw_overview(frame, app, content);
        }
        Screen::Tasks => {
            draw_task_list(frame, app, side);
            draw_task_pane(frame, app, content);
        }
    }
    draw_status_bar(frame, app, bar);
    if app.help {
        draw_help(frame);
    }
}

// ── glyphs ──────────────────────────────────────────────────────────────

fn task_glyph(status: TaskStatus, tick: usize) -> (&'static str, Style) {
    match status {
        TaskStatus::Todo => ("○", Style::default().fg(Color::Gray)),
        TaskStatus::Working => (
            SPINNER[tick % SPINNER.len()],
            Style::default().fg(WORKING).add_modifier(Modifier::BOLD),
        ),
        TaskStatus::Paused => ("‖", Style::default().fg(Color::Yellow)),
        TaskStatus::Blocked => ("■", Style::default().fg(Color::LightRed)),
        TaskStatus::Done => ("✔", Style::default().fg(Color::Green)),
        TaskStatus::Skipped => ("–", dim()),
    }
}

/// A project spins while any of its tasks is being worked; otherwise its
/// status shows as a fixed glyph.
fn project_glyph(project: &Project, tick: usize) -> (&'static str, Style) {
    if project.working().next().is_some() {
        return task_glyph(TaskStatus::Working, tick);
    }
    match project.meta.status {
        ProjectStatus::Planning => ("◇", Style::default().fg(Color::LightBlue)),
        ProjectStatus::Active => ("●", Style::default().fg(Color::Cyan)),
        ProjectStatus::Paused => ("‖", Style::default().fg(Color::Yellow)),
        ProjectStatus::Done => ("✔", Style::default().fg(Color::Green)),
        ProjectStatus::Dropped => ("✖", dim()),
    }
}

fn task_color(status: TaskStatus) -> Color {
    match status {
        TaskStatus::Todo => Color::Gray,
        TaskStatus::Working => WORKING,
        TaskStatus::Paused => Color::Yellow,
        TaskStatus::Blocked => Color::LightRed,
        TaskStatus::Done => Color::Green,
        TaskStatus::Skipped => Color::DarkGray,
    }
}

// ── left pane ───────────────────────────────────────────────────────────

fn draw_project_list(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let block = pane_block(format!(" Projects ({}) ", app.projects.len()), true);
    if app.projects.is_empty() {
        let hint = Text::from(vec![
            Line::styled(
                "No projects yet.",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Line::default(),
            Line::raw("Ask an agent to start one with"),
            Line::raw("the /scout skill, or run:"),
            Line::default(),
            Line::styled(
                "  beagle scout new <id> --title \"...\"",
                Style::default().fg(Color::Cyan),
            ),
        ]);
        frame.render_widget(
            Paragraph::new(hint).block(block).wrap(Wrap { trim: false }),
            area,
        );
        return;
    }
    let width = usize::from(area.width.saturating_sub(4));
    let items: Vec<ListItem<'static>> = app
        .projects
        .iter()
        .map(|p| project_item(p, app.tick, width))
        .collect();
    render_list(frame, area, block, items, app.project_sel);
}

fn project_item(project: &Project, tick: usize, width: usize) -> ListItem<'static> {
    let (glyph, style) = project_glyph(project, tick);
    let mut meta = vec![Span::styled(
        format!(
            "  {} · {}/{} done",
            project.meta.status.as_str(),
            project.closed_count(),
            project.tasks.len()
        ),
        dim(),
    )];
    if project.working().next().is_some() {
        if let Some((label, quiet)) = activity(project.last_activity) {
            meta.push(Span::styled(" · ", dim()));
            meta.push(Span::styled(
                label,
                if quiet {
                    Style::default().fg(Color::Yellow)
                } else {
                    dim()
                },
            ));
        }
    }
    ListItem::new(vec![
        Line::from(vec![
            Span::styled(glyph, style),
            Span::raw(" "),
            Span::raw(truncate(&project.meta.title, width.saturating_sub(2))),
        ]),
        Line::from(meta),
    ])
}

fn draw_task_list(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let Some(project) = app.selected_project() else {
        return;
    };
    let width = usize::from(area.width.saturating_sub(4));
    let title = format!(
        " ‹ {} ",
        truncate(&project.meta.title, width.saturating_sub(4))
    );
    let block = pane_block(title, app.focus == Focus::List);
    if project.tasks.is_empty() {
        let hint = Text::from(vec![
            Line::styled(
                "No tasks yet.",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Line::default(),
            Line::styled(
                "  beagle scout task add <project> <slug> --title \"...\"",
                Style::default().fg(Color::Cyan),
            ),
        ]);
        frame.render_widget(
            Paragraph::new(hint).block(block).wrap(Wrap { trim: false }),
            area,
        );
        return;
    }
    let items: Vec<ListItem<'static>> = project
        .tasks
        .iter()
        .map(|t| task_item(t, app.tick, width))
        .collect();
    render_list(frame, area, block, items, app.task_sel);
}

fn task_item(task: &Task, tick: usize, width: usize) -> ListItem<'static> {
    let (glyph, style) = task_glyph(task.meta.status, tick);
    let closed = task.meta.status.is_closed();
    let title_style = if closed { dim() } else { Style::default() };
    let mut meta = vec![Span::styled(
        format!("     {}", task.meta.status.as_str()),
        Style::default().fg(task_color(task.meta.status)),
    )];
    let (checked, total) = task.checklist;
    if total > 0 {
        meta.push(Span::styled(format!(" · {checked}/{total} ☑"), dim()));
    }
    if task.meta.status == TaskStatus::Working {
        if let Some((label, quiet)) = activity(task.modified) {
            meta.push(Span::styled(" · ", dim()));
            meta.push(Span::styled(
                label,
                if quiet {
                    Style::default().fg(Color::Yellow)
                } else {
                    dim()
                },
            ));
        }
    }
    ListItem::new(vec![
        Line::from(vec![
            Span::styled(glyph, style),
            Span::styled(format!(" {:02} ", task.number), dim()),
            Span::styled(
                truncate(&task.meta.title, width.saturating_sub(5)),
                title_style,
            ),
        ]),
        Line::from(meta),
    ])
}

fn render_list(
    frame: &mut Frame<'_>,
    area: Rect,
    block: Block<'static>,
    items: Vec<ListItem<'static>>,
    selected: usize,
) {
    let list = List::new(items).block(block).highlight_style(
        Style::default()
            .bg(Color::Rgb(45, 45, 60))
            .add_modifier(Modifier::BOLD),
    );
    let mut state = ListState::default().with_selected(Some(selected));
    frame.render_stateful_widget(list, area, &mut state);
}

// ── chrome ──────────────────────────────────────────────────────────────

fn draw_status_bar(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let mut spans = Vec::new();
    if app.follow {
        spans.push(Span::styled(
            " FOLLOW ",
            Style::default().fg(Color::Black).bg(Color::Cyan),
        ));
        spans.push(Span::raw(" "));
    }
    if let Some(message) = &app.status {
        spans.push(Span::styled(
            message.clone(),
            Style::default().fg(Color::Yellow),
        ));
    } else {
        let hints = match (app.screen, app.focus) {
            (Screen::Projects, _) => "j/k select · enter open · ctrl-d/u scroll · E edit · ? help · Q quit",
            (Screen::Tasks, Focus::List) => {
                "j/k select · enter read · tab/1-5 tabs · b back · F follow · E edit · ? help · Q quit"
            }
            (Screen::Tasks, Focus::Content) => {
                "j/k scroll · space page · g/G top/bottom · tab/1-5 tabs · b back · F follow · E edit · ?"
            }
        };
        spans.push(Span::styled(hints, dim()));
    }
    if !app.warnings.is_empty() {
        spans.push(Span::styled(
            format!(
                "  ⚠ {} file(s) failed to load — `beagle scout list` shows why",
                app.warnings.len()
            ),
            Style::default().fg(Color::LightRed),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_help(frame: &mut Frame<'_>) {
    let keys_width = HELP
        .iter()
        .map(|(k, _)| k.chars().count())
        .max()
        .unwrap_or(0);
    let mut lines: Vec<Line<'static>> = HELP
        .iter()
        .map(|(keys, what)| {
            Line::from(vec![
                Span::styled(
                    format!(" {keys:<keys_width$}  "),
                    Style::default().fg(Color::Yellow),
                ),
                Span::raw(*what),
            ])
        })
        .collect();
    lines.push(Line::default());
    lines.push(Line::styled(" any key closes this", dim()));
    let area = frame.area();
    let height = u16::try_from(lines.len() + 2)
        .unwrap_or(u16::MAX)
        .min(area.height);
    let width = 84.min(area.width);
    let popup = Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    };
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(lines)
            .block(pane_block(" keys ".to_owned(), true))
            .wrap(Wrap { trim: false }),
        popup,
    );
}

fn pane_block(title: String, focused: bool) -> Block<'static> {
    let border = if focused {
        Color::Yellow
    } else {
        Color::DarkGray
    };
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border))
}

/// `active 2m ago`, or `quiet 12m` (flagged) once past [`QUIET_AFTER`].
fn activity(last: Option<SystemTime>) -> Option<(String, bool)> {
    let age = SystemTime::now().duration_since(last?).unwrap_or_default();
    let minutes = age.as_secs() / 60;
    let span = if minutes < 60 {
        format!("{minutes}m")
    } else {
        format!("{}h{:02}m", minutes / 60, minutes % 60)
    };
    let quiet = age >= QUIET_AFTER;
    Some(if quiet {
        (format!("quiet {span}"), true)
    } else {
        (format!("active {span} ago"), false)
    })
}

use beagle_utils::text::truncate_chars as truncate;

mod pane;

pub(in crate::ui) use pane::extend_layout;
use pane::{draw_overview, draw_task_pane};

#[cfg(test)]
mod tests;
