//! The right pane: overview and task panes, headers, tabs, and the
//! wrapped, scrolled content.

use super::{
    activity, dim, pane_block, project_glyph, task_color, task_glyph, App, Block, Color,
    Constraint, Focus, Frame, Layout, Line, Modifier, Paragraph, Project, Rect, Span, Style, Tab,
    Task, TaskStatus, Text, Wrap, WrapLayout, SPINNER, WORKING,
};

pub(super) fn draw_overview(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    let Some(project) = app.selected_project() else {
        frame.render_widget(pane_block(" scout ".to_owned(), false), area);
        return;
    };
    let title = format!(" {} ", project.meta.title);
    let mut lines = project_header(project, app.tick);
    lines.push(Line::default());
    render_scrolled(frame, app, area, pane_block(title, false), lines);
}

pub(super) fn draw_task_pane(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    let [tabs, body] = Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(area);
    frame.render_widget(Paragraph::new(tab_line(app.tab)), tabs);

    let Some(project) = app.selected_project() else {
        return;
    };
    let (title, mut lines) = match app.tab {
        Tab::Task | Tab::Outcome | Tab::Session => match app.selected_task() {
            Some(task) => {
                let what = match app.tab {
                    Tab::Outcome => "Outcome · ",
                    Tab::Session => "Session · ",
                    _ => "",
                };
                (
                    format!(" {what}{} · {} ", task.id, task.meta.title),
                    task_header(task, project.tasks.len(), app.tick),
                )
            }
            None => (" Task ".to_owned(), Vec::new()),
        },
        Tab::Project => (
            format!(" {} ", project.meta.title),
            project_header(project, app.tick),
        ),
        Tab::Log => (format!(" Log · {} ", project.id), Vec::new()),
    };
    if !lines.is_empty() {
        lines.push(Line::default());
    }
    let focused = app.focus == Focus::Content;
    render_scrolled(frame, app, body, pane_block(title, focused), lines);
}

pub(super) fn tab_line(current: Tab) -> Line<'static> {
    let mut spans = vec![Span::raw(" ")];
    for (i, tab) in Tab::ALL.into_iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" · ", dim()));
        }
        let label = format!(" {} {} ", i + 1, tab.label());
        spans.push(if tab == current {
            Span::styled(
                label,
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled(label, Style::default().fg(Color::Gray))
        });
    }
    Line::from(spans)
}

/// Status, progress bar, what is being worked now (or what is next), what
/// is blocked, and links: the "where are we?" summary of a project.
pub(super) fn project_header(project: &Project, tick: usize) -> Vec<Line<'static>> {
    let (glyph, style) = project_glyph(project, tick);
    let total = project.tasks.len();
    let closed = project.closed_count();
    let mut lines = vec![Line::from(vec![
        Span::styled(glyph, style),
        Span::raw(" "),
        Span::styled(
            project.meta.status.as_str(),
            style.add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(
                " · {closed}/{total} tasks closed · started {}",
                project.meta.created.date()
            ),
            dim(),
        ),
    ])];
    lines.push(progress_bar(closed, total));

    let mut working = project.working().peekable();
    if working.peek().is_none() {
        if let Some(next) = project.next_todo() {
            lines.push(Line::from(vec![
                Span::styled("  next  ", Style::default().fg(Color::Cyan)),
                Span::styled(format!("{} ", next.id), dim()),
                Span::raw(next.meta.title.clone()),
            ]));
        } else if total == 0 {
            lines.push(Line::styled("  no tasks yet", dim()));
        }
    }
    for task in working {
        let mut spans = vec![
            Span::styled(
                format!("{} now   ", SPINNER[tick % SPINNER.len()]),
                Style::default().fg(WORKING).add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("{} ", task.id), dim()),
            Span::styled(
                task.meta.title.clone(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ];
        if let Some((label, quiet)) = activity(task.modified.max(project.last_activity)) {
            spans.push(Span::styled(" · ", dim()));
            spans.push(Span::styled(
                label,
                if quiet {
                    Style::default().fg(Color::Yellow)
                } else {
                    dim()
                },
            ));
        }
        lines.push(Line::from(spans));
    }
    for task in project
        .tasks
        .iter()
        .filter(|t| t.meta.status == TaskStatus::Blocked)
    {
        lines.push(Line::from(vec![
            Span::styled("■ blocked ", Style::default().fg(Color::LightRed)),
            Span::styled(format!("{} ", task.id), dim()),
            Span::raw(task.meta.title.clone()),
        ]));
    }
    if !project.meta.links.is_empty() {
        lines.push(links_line(&project.meta.links));
    }
    lines
}

pub(super) fn task_header(task: &Task, of: usize, tick: usize) -> Vec<Line<'static>> {
    let (glyph, style) = task_glyph(task.meta.status, tick);
    let mut first = vec![
        Span::styled(glyph, style),
        Span::raw(" "),
        Span::styled(
            task.meta.status.as_str(),
            Style::default()
                .fg(task_color(task.meta.status))
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" · task {} of {of}", task.number), dim()),
    ];
    let (checked, total) = task.checklist;
    if total > 0 {
        first.push(Span::styled(
            format!(" · checklist {checked}/{total}"),
            dim(),
        ));
    }
    if let Some(updated) = task.meta.updated {
        first.push(Span::styled(
            format!(
                " · updated {} {:02}:{:02} UTC",
                updated.date(),
                updated.hour(),
                updated.minute()
            ),
            dim(),
        ));
    }
    if task.meta.status == TaskStatus::Working {
        if let Some((label, quiet)) = activity(task.modified) {
            first.push(Span::styled(" · ", dim()));
            first.push(Span::styled(
                label,
                if quiet {
                    Style::default().fg(Color::Yellow)
                } else {
                    dim()
                },
            ));
        }
    }
    let mut lines = vec![Line::from(first)];
    if !task.meta.links.is_empty() {
        lines.push(links_line(&task.meta.links));
    }
    lines
}

pub(super) fn links_line(links: &[String]) -> Line<'static> {
    Line::from(vec![
        Span::styled("  links ", Style::default().fg(Color::Cyan)),
        Span::raw(links.join(" · ")),
    ])
}

pub(super) fn progress_bar(closed: usize, total: usize) -> Line<'static> {
    const WIDTH: usize = 30;
    let filled = (closed * WIDTH).checked_div(total).unwrap_or(0);
    let percent = (closed * 100).checked_div(total).unwrap_or(0);
    Line::from(vec![
        Span::raw("  "),
        Span::styled("█".repeat(filled), Style::default().fg(Color::Green)),
        Span::styled("░".repeat(WIDTH - filled), dim()),
        Span::styled(format!(" {percent}%"), dim()),
    ])
}

/// Renders a pane: `header` (a few status lines, built per frame) followed
/// by the cached pane text, wrapped, scrolled, and clamped.
///
/// Only the rows on screen are wrapped and drawn. The pane text's wrapped
/// layout is computed once per content or width change ([`WrapLayout`]), so a
/// frame costs the same for a 30-line task and a 30,000-line session.
pub(super) fn render_scrolled(
    frame: &mut Frame<'_>,
    app: &mut App,
    area: Rect,
    block: Block<'static>,
    header: Vec<Line<'static>>,
) {
    let inner = block.inner(area);
    let width = inner.width;
    let height = usize::from(inner.height);
    if let Some(pane) = app.pane.as_mut() {
        if pane.layout.as_ref().map(|l| l.width) != Some(width) {
            pane.layout = Some(layout(&pane.text.lines, width));
        }
    }
    let header_rows: Vec<usize> = header.iter().map(|l| rows(l, width)).collect();
    let header_total: usize = header_rows.iter().sum();
    let pane = app.pane.as_ref();
    let pane_starts = pane
        .and_then(|p| p.layout.as_ref())
        .map(|l| l.starts.as_slice());
    let pane_total = pane_starts.and_then(|s| s.last().copied()).unwrap_or(0);

    app.max_scroll = (header_total + pane_total).saturating_sub(height);
    app.viewport_height = inner.height;
    app.scroll = app.scroll.min(app.max_scroll);
    let (top, bottom) = (app.scroll, app.scroll + height);

    // Collect the lines overlapping rows [top, bottom), and how many rows of
    // the first one are above the top.
    let mut visible: Vec<Line<'static>> = Vec::new();
    let mut skip = 0;
    let mut row = 0;
    for (line, h) in header.into_iter().zip(header_rows) {
        if row + h > top && row < bottom {
            if visible.is_empty() {
                skip = top - row;
            }
            visible.push(line);
        }
        row += h;
    }
    if let (Some(pane), Some(starts)) = (pane, pane_starts) {
        let lines = &pane.text.lines;
        let local_top = top.saturating_sub(header_total);
        // The first pane line ending below the top.
        let first = starts
            .partition_point(|&s| s <= local_top)
            .saturating_sub(1);
        for (i, line) in lines.iter().enumerate().skip(first) {
            let start = header_total + starts[i];
            if start >= bottom {
                break;
            }
            let end = header_total + starts[i + 1];
            if end > top {
                if visible.is_empty() {
                    skip = top - start;
                }
                visible.push(line.clone());
            }
        }
    }
    let skip = u16::try_from(skip).unwrap_or(u16::MAX);
    let paragraph = Paragraph::new(Text::from(visible))
        .wrap(Wrap { trim: false })
        .scroll((skip, 0))
        .block(block);
    frame.render_widget(paragraph, area);
}

/// Wrapped-row offsets for `lines` at `width`.
pub(super) fn layout(lines: &[Line<'static>], width: u16) -> WrapLayout {
    let mut starts = Vec::with_capacity(lines.len() + 1);
    let mut row = 0;
    starts.push(0);
    for line in lines {
        row += rows(line, width);
        starts.push(row);
    }
    WrapLayout { width, starts }
}

/// Reuses `layout` (computed for `old`) for `new` when `new` only adds
/// lines at the end, wrapping just the added ones; `None` when anything
/// earlier changed (then the next frame computes it from scratch).
pub(in crate::ui) fn extend_layout(
    mut layout: WrapLayout,
    old: &[Line<'static>],
    new: &[Line<'static>],
) -> Option<WrapLayout> {
    if new.len() < old.len() || new[..old.len()] != *old {
        return None;
    }
    let mut row = layout.starts.last().copied().unwrap_or(0);
    for line in &new[old.len()..] {
        row += rows(line, layout.width);
        layout.starts.push(row);
    }
    Some(layout)
}

/// How many rows `line` takes when wrapped to `width` (at least one).
pub(super) fn rows(line: &Line<'static>, width: u16) -> usize {
    if width == 0 {
        return 1;
    }
    Paragraph::new(line.clone())
        .wrap(Wrap { trim: false })
        .line_count(width)
        .max(1)
}
