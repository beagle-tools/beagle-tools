//! The jobs screens.
//!
//! **Jobs** (the landing page): every job, each with its whole flow drawn
//! beneath it, and the selected job's manifest beside. The cursor moves
//! job to job — the steps are the picture, not stops. `/` narrows the
//! list.
//!
//! **Runs** (enter on a job): that job's runs, newest first, each opened
//! by a ruled partition so one run never reads into the next. The cursor
//! moves step to step; enter opens a step's log, and the pane beside
//! shows the step's details.
//!
//! The model behind both is in [`crate::jobs`].

use agent_core::types::AgentSpec;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::App;
use crate::format::ago_coarse;
use crate::jobs::JobEntry;
pub(crate) use crate::widgets::{badge, hints, pane_block, state_style, tree_prefixes, window};

const JOBS_HINTS: &str =
    "j/k:job  enter:runs  A:all runs  R:run  y:copy name  r:running  a:all  /:search  e:edit  q:quit  ?:help";

pub fn draw(frame: &mut Frame<'_>, app: &App) {
    if app.runs_open() {
        crate::jobruns::draw(frame, app);
    } else {
        draw_jobs_screen(frame, app);
    }
}

// ---- jobs screen --------------------------------------------------------

fn draw_jobs_screen(frame: &mut Frame<'_>, app: &App) {
    let [header, main] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(3)]).areas(frame.area());
    let [list, config] =
        Layout::horizontal([Constraint::Percentage(45), Constraint::Percentage(55)]).areas(main);

    let jobs = app.jobs();
    let mut spans = vec![
        badge(" jobs "),
        Span::raw(format!(
            "  {} job{}  ",
            jobs.len(),
            if jobs.len() == 1 { "" } else { "s" }
        )),
    ];
    if app.job_running_only {
        spans.push(Span::styled(
            " running only ",
            Style::new()
                .fg(Color::Black)
                .bg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw("  "));
    }
    if app.show_hidden && !app.view.hidden_projects.is_empty() {
        spans.push(Span::styled(
            " + hidden ",
            Style::new()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw("  "));
    } else {
        let hidden = app.hidden_job_count();
        if hidden > 0 {
            spans.push(Span::styled(
                format!("{hidden} hidden (H)  "),
                Style::new().add_modifier(Modifier::DIM),
            ));
        }
    }
    if app.job_search_input || !app.job_search.is_empty() {
        let cursor = if app.job_search_input { "▏" } else { "" };
        spans.push(Span::styled(
            format!(" /{}{cursor} ", app.job_search),
            Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            if app.job_search_input {
                "  enter:keep  esc:clear"
            } else {
                "  esc:clear"
            },
            Style::new().add_modifier(Modifier::DIM),
        ));
    } else {
        spans.push(hints(app, JOBS_HINTS));
    }
    // The store failing to open or query used to show in the old dashboard's
    // ACTIVE pane; here it leads the header, or it would be invisible.
    if let Some(err) = app.store_error.as_ref().or(app.view_error.as_ref()) {
        spans.insert(
            1,
            Span::styled(
                format!(" {err} "),
                Style::new()
                    .fg(Color::White)
                    .bg(Color::Red)
                    .add_modifier(Modifier::BOLD),
            ),
        );
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), header);

    draw_job_list(frame, app, &jobs, list);
    draw_config(frame, app.dag_selected_agent(), config);
}

fn draw_job_list(frame: &mut Frame<'_>, app: &App, jobs: &[JobEntry<'_>], area: Rect) {
    let now = chrono::Utc::now();
    // With agentd down nothing on this list will fire: the names go red,
    // like the bar at the bottom.
    let stopped = app.daemon.is_some_and(|d| !d.is_running());
    let mut lines: Vec<Line<'_>> = Vec::new();
    let mut selected_line = 0usize;
    let mut project = "";

    for (idx, job) in jobs.iter().enumerate() {
        if job.spec.project != project {
            project = &job.spec.project;
            if !lines.is_empty() {
                lines.push(Line::raw(""));
            }
            lines.push(Line::from(Span::styled(
                project.to_owned(),
                Style::new().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            )));
        }
        let selected = idx == app.dag_selected;
        if selected {
            selected_line = lines.len();
        }
        let mut row = job_line(job, selected, stopped, now);
        // Collapsed unless the cursor is on it: a long chain's tree for
        // every job buried the list. The others say how many steps they
        // hold; moving onto one opens its flow.
        if !selected {
            let steps: std::collections::BTreeSet<&str> =
                job.tree.iter().skip(1).map(|(a, _)| a.as_str()).collect();
            if !steps.is_empty() && job.spec.enabled {
                row.spans.push(Span::styled(
                    format!(
                        "  · {} step{}",
                        steps.len(),
                        if steps.len() == 1 { "" } else { "s" }
                    ),
                    Style::new().add_modifier(Modifier::DIM),
                ));
            }
            lines.push(row);
            continue;
        }
        lines.push(row);

        // The flow, beneath the job it belongs to. Not selectable: the
        // cursor goes job to job, and this is the picture of each.
        let depths: Vec<usize> = job.tree.iter().map(|(_, d)| *d).collect();
        let prefixes = tree_prefixes(&depths);
        for ((agent, _), prefix) in job.tree.iter().zip(&prefixes).skip(1) {
            let live = job.running.iter().any(|r| r == agent);
            lines.push(Line::from(vec![
                Span::raw("    "),
                Span::styled(prefix.clone(), Style::new().add_modifier(Modifier::DIM)),
                Span::styled(
                    agent.clone(),
                    if live {
                        Style::new().fg(Color::Green).add_modifier(Modifier::BOLD)
                    } else {
                        Style::new().fg(Color::Cyan).add_modifier(Modifier::DIM)
                    },
                ),
                Span::styled(
                    if live { "  ● running" } else { "" },
                    Style::new().fg(Color::Green),
                ),
            ]));
        }
        if job.tree.len() <= 1 && job.last.is_none() {
            lines.push(Line::from(Span::styled(
                "    (no runs yet — its steps appear once it has run)",
                Style::new().add_modifier(Modifier::DIM),
            )));
        }
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            if app.job_running_only {
                "nothing running right now — a shows every job"
            } else if app.job_search.is_empty() {
                "no jobs — drop a manifest in manifests/<project>/ and it registers in seconds"
            } else {
                "no job matches that search — esc clears it"
            },
            Style::new().add_modifier(Modifier::DIM),
        )));
    }

    frame.render_widget(
        Paragraph::new(window(lines, selected_line, area)).block(pane_block("JOBS")),
        area,
    );
}

fn job_line<'a>(
    job: &JobEntry<'a>,
    selected: bool,
    stopped: bool,
    now: chrono::DateTime<chrono::Utc>,
) -> Line<'a> {
    let trigger = job
        .spec
        .schedule
        .as_deref()
        .map_or_else(|| "manual".to_owned(), |s| format!("cron {s}"));
    let mut spans = vec![
        Span::styled(
            // ▾ open (the cursor is on it), ▸ collapsed, ● running.
            if !job.running.is_empty() {
                "● "
            } else if !job.spec.enabled {
                "⏸ "
            } else if selected {
                "▾ "
            } else {
                "▸ "
            },
            Style::new().fg(if job.running.is_empty() {
                Color::Reset
            } else {
                Color::Green
            }),
        ),
        Span::styled(
            job.spec.name.clone(),
            Style::new()
                // Paused reads as off at a glance: grey, not the live cyan.
                .fg(if !job.spec.enabled {
                    Color::DarkGray
                } else if stopped {
                    Color::Red
                } else {
                    Color::Cyan
                })
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            // The runner is in the config pane; the row keeps to what fits.
            format!("  {trigger}"),
            Style::new().add_modifier(Modifier::DIM),
        ),
    ];
    if !job.spec.enabled {
        spans.push(Span::styled(
            "  OFF",
            Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ));
    }
    // While it runs, WHERE it is matters more than how it last ended.
    if job.running.is_empty() {
        match job.last {
            Some(run) => spans.push(Span::styled(
                format!("  {} {} ago", run.state, ago_coarse(run.created_at, now)),
                state_style(run.state),
            )),
            None => spans.push(Span::styled(
                "  never run",
                Style::new().add_modifier(Modifier::DIM),
            )),
        }
    } else {
        spans.push(Span::styled(
            format!("  running: {}", job.running.join(", ")),
            Style::new().fg(Color::Green).add_modifier(Modifier::BOLD),
        ));
    }
    let line = Line::from(spans);
    if selected {
        line.style(Style::new().add_modifier(Modifier::REVERSED))
    } else {
        line
    }
}

// ---- shared ------------------------------------------------------------

/// The manifest that declared the job, verbatim.
fn draw_config(frame: &mut Frame<'_>, agent: Option<&AgentSpec>, area: Rect) {
    let (title, lines) = match agent {
        None => (
            "CONFIG".to_owned(),
            vec![Line::from(Span::styled(
                "no job selected",
                Style::new().add_modifier(Modifier::DIM),
            ))],
        ),
        Some(agent) => {
            let name = agent
                .source
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("manifest");
            let lines = match std::fs::read_to_string(&agent.source) {
                Ok(body) => crate::highlight::yaml(&body),
                Err(e) => vec![Line::from(format!(
                    "cannot read {}: {e}",
                    agent.source.display()
                ))],
            };
            (format!("CONFIG · {name} · e to edit"), lines)
        }
    };
    let mut lines = lines;
    lines.truncate(usize::from(area.height.saturating_sub(2)));
    frame.render_widget(Paragraph::new(lines).block(pane_block(&title)), area);
}

#[cfg(test)]
#[path = "tests/dag.rs"]
mod tests;
