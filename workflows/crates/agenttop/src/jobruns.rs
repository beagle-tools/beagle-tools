//! One job's runs screen: every run of the job, newest first, each opened
//! by a ruled partition, its steps beneath as a tree. Enter on a step
//! opens its log; the pane beside shows the step's details.
//!
//! Split from [`crate::dag`], which draws the jobs list and owns the
//! shared helpers used here.

use agent_core::types::Run;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::App;
use crate::dag::{badge, hints, pane_block, state_style, tree_prefixes, window};
use crate::format::{cost, elapsed_between};
use crate::jobs::RunRow;

const RUNS_HINTS: &str = "j/k:step  enter:logs  m:details  esc:back";
/// Width of the guide + step name column, so state lines up down the
/// whole tree whatever the depth.
const NAME_COL: usize = 34;

pub fn draw(frame: &mut Frame<'_>, app: &App) {
    let [header, main] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(3)]).areas(frame.area());
    let [runs_area, detail_area] =
        Layout::horizontal([Constraint::Percentage(62), Constraint::Percentage(38)]).areas(main);

    let rows = app.job_run_rows();
    let run_count = rows
        .iter()
        .filter(|r| matches!(r, RunRow::Header { .. }))
        .count();
    let (project, name) = app
        .job_open
        .as_ref()
        .map_or(("", ""), |(p, n)| (p.as_str(), n.as_str()));

    // The job's name lives up here, once, so the rows below can be just
    // the steps.
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            badge(" jobs "),
            Span::styled(
                format!(" › {project}/{name} "),
                Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(" {run_count} runs  ")),
            hints(app, RUNS_HINTS),
        ])),
        header,
    );

    let inner_width = usize::from(runs_area.width.saturating_sub(2));
    let now = chrono::Utc::now();
    let mut lines: Vec<Line<'_>> = Vec::new();
    let mut selected_line = 0usize;
    let mut step_index = 0usize;
    let mut chain_depths: Vec<usize> = Vec::new();
    let mut chain_lines_at = 0usize;

    // Steps are drawn as they arrive, but their tree guides need the
    // whole chain's depths first — so collect a chain, then flush it.
    // The guide and the name share ONE fixed-width column, padded
    // together. Padding the name alone and inserting the guide before it
    // pushed the state column right by three cells per level of depth.
    let flush = |lines: &mut Vec<Line<'_>>, at: usize, depths: &[usize]| {
        for (offset, prefix) in tree_prefixes(depths).into_iter().enumerate() {
            if let Some(line) = lines.get_mut(at + offset) {
                if let Some(name) = line.spans.get_mut(1) {
                    let agent = name.content.trim_end().to_owned();
                    let width = NAME_COL.saturating_sub(prefix.chars().count());
                    name.content = format!("{agent:<width$}").into();
                }
                line.spans.insert(
                    1,
                    Span::styled(prefix, Style::new().add_modifier(Modifier::DIM)),
                );
            }
        }
    };

    for row in &rows {
        match row {
            RunRow::Header { root, cost: total } => {
                flush(&mut lines, chain_lines_at, &chain_depths);
                chain_depths.clear();
                if !lines.is_empty() {
                    lines.push(Line::raw(""));
                }
                lines.push(partition(
                    root,
                    total.total,
                    total.partial,
                    inner_width,
                    now,
                ));
                chain_lines_at = lines.len();
            }
            RunRow::Step { run, depth } => {
                chain_depths.push(*depth);
                let selected = step_index == app.job_step_selected;
                if selected {
                    selected_line = lines.len();
                }
                lines.push(step_line(run, selected, now));
                step_index += 1;
            }
        }
    }
    flush(&mut lines, chain_lines_at, &chain_depths);

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            format!("{name} has not run yet — start it with: ag -j {name}"),
            Style::new().add_modifier(Modifier::DIM),
        )));
    }

    frame.render_widget(
        Paragraph::new(window(lines, selected_line, runs_area))
            .block(pane_block(&format!("RUNS · {name}"))),
        runs_area,
    );

    // The step's details beside the list — what it is before you open
    // the log of it.
    let detail = match app.dag_selected_run() {
        Some(run) => {
            let mut lines = crate::detail::run_detail(run);
            lines.truncate(usize::from(detail_area.height.saturating_sub(2)));
            Paragraph::new(lines).block(pane_block(&format!("STEP · {}", run.agent)))
        }
        None => Paragraph::new(Line::from(Span::styled(
            "no step selected",
            Style::new().add_modifier(Modifier::DIM),
        )))
        .block(pane_block("STEP")),
    };
    frame.render_widget(detail, detail_area);
}

/// The ruled line that opens one run: when it started, how it ended, how
/// long ago, what the whole chain cost. Ruled to the pane's width, so the
/// boundary between two runs is unmissable.
fn partition<'a>(
    root: &Run,
    total: f64,
    partial: bool,
    width: usize,
    now: chrono::DateTime<chrono::Utc>,
) -> Line<'a> {
    let when = root
        .created_at
        .with_timezone(&chrono::Local)
        .format("%b %d %H:%M");
    let spend = if total > 0.0 {
        format!(
            " · Σ {}{}",
            cost(Some(total)),
            if partial { "+" } else { "" }
        )
    } else {
        String::new()
    };
    let label = format!(
        "━━ {when} · {} · {} ago{spend} ",
        root.state,
        elapsed_between(root.created_at, now)
    );
    let fill = width.saturating_sub(label.chars().count());
    Line::from(vec![
        Span::styled(label, state_style(root.state).add_modifier(Modifier::BOLD)),
        Span::styled("━".repeat(fill), state_style(root.state)),
    ])
}

/// One step. Index 1 is left free for the tree guide, filled in once the
/// whole chain is known.
fn step_line<'a>(run: &Run, selected: bool, now: chrono::DateTime<chrono::Utc>) -> Line<'a> {
    let took = match run.started_at {
        Some(start) => elapsed_between(start, run.finished_at.unwrap_or(now)),
        None => "—".to_owned(),
    };
    let line = Line::from(vec![
        Span::raw("  "),
        Span::styled(run.agent.clone(), Style::new().fg(Color::Cyan)),
        Span::styled(
            format!("{:<11}", run.state.to_string()),
            state_style(run.state),
        ),
        Span::styled(
            format!("{took:>8}"),
            Style::new().add_modifier(Modifier::DIM),
        ),
        Span::styled(
            run.cost_usd
                .map_or_else(String::new, |c| format!("  {}", cost(Some(c)))),
            Style::new().add_modifier(Modifier::DIM),
        ),
    ]);
    if selected {
        line.style(Style::new().add_modifier(Modifier::REVERSED))
    } else {
        line
    }
}
