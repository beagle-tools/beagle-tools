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

const RUNS_HINTS: &str =
    "j/k:step  enter:logs  y:copy id  W:watch  w:write-up  o:open PR  m:details  R:resume/run  x:kill  esc:back";
const ALL_RUNS_HINTS: &str =
    "j/k:step  enter:logs  y:copy id  W:watch  r:running  a:all  w:write-up  o:PR  R:resume/run  esc:back";
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

    draw_header(frame, app, header, (project, name), run_count);

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
                    app.all_runs,
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
        let empty = if app.all_runs && app.all_running_only {
            "nothing is running right now — a to show every chain".to_owned()
        } else if app.all_runs {
            "no runs yet".to_owned()
        } else {
            format!("{name} has not run yet — start it with: ag -j {name}")
        };
        lines.push(Line::from(Span::styled(
            empty,
            Style::new().add_modifier(Modifier::DIM),
        )));
    }

    let title = if app.all_runs {
        "RUNS · all jobs".to_owned()
    } else {
        format!("RUNS · {name}")
    };
    frame.render_widget(
        Paragraph::new(window(lines, selected_line, runs_area)).block(pane_block(&title)),
        runs_area,
    );

    draw_step_pane(frame, app, detail_area);
}

/// The one-line header: which job (or all of them), how many runs, the
/// running-only badge, and the keys.
fn draw_header(
    frame: &mut Frame<'_>,
    app: &App,
    header: ratatui::layout::Rect,
    (project, name): (&str, &str),
    run_count: usize,
) {
    // The job's name lives up here, once, so the rows below can be just
    // the steps. On the all-runs page each partition names its job instead.
    let mut head = vec![
        badge(" jobs "),
        Span::styled(
            if app.all_runs {
                " › all runs ".to_owned()
            } else {
                format!(" › {project}/{name} ")
            },
            Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!(" {run_count} runs  ")),
    ];
    if app.follow_top {
        head.push(Span::styled(
            " watching ",
            Style::new()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
        head.push(Span::raw("  "));
    }
    if app.all_runs && app.all_running_only {
        head.push(Span::styled(
            " running only ",
            Style::new()
                .fg(Color::Black)
                .bg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ));
        head.push(Span::raw("  "));
    }
    head.push(hints(
        app,
        if app.all_runs {
            ALL_RUNS_HINTS
        } else {
            RUNS_HINTS
        },
    ));
    frame.render_widget(Paragraph::new(Line::from(head)), header);
}

/// The selected step's details beside the runs list — what it is (and the
/// PR it opened) before you open its log.
fn draw_step_pane(frame: &mut Frame<'_>, app: &App, detail_area: ratatui::layout::Rect) {
    // The step's details beside the list — what it is before you open
    // the log of it.
    let detail = match app.dag_selected_run() {
        Some(run) => {
            let mut lines = crate::detail::run_detail(run);
            // The PR first: it is what a step on a code ticket produced,
            // and the thing you most want from its metadata.
            if let Some(pr) = app.selected_step_pr() {
                lines.insert(
                    0,
                    Line::from(vec![
                        Span::styled("draft PR    ", Style::new().add_modifier(Modifier::DIM)),
                        Span::styled(
                            pr.to_owned(),
                            Style::new().fg(Color::Green).add_modifier(Modifier::BOLD),
                        ),
                    ]),
                );
                lines.insert(1, Line::raw(""));
            }
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
    name_job: bool,
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
    // On the all-runs page the partition is the only place the job is named.
    let job = if name_job {
        format!("{}/{} · ", root.project, root.agent)
    } else {
        String::new()
    };
    let label = format!(
        "━━ {job}{when} · {} · {} ago{spend} ",
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
