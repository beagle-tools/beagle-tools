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

use agent_core::types::{AgentSpec, RunState};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

use crate::app::App;
use crate::format::elapsed_between;
use crate::jobs::JobEntry;

const JOBS_HINTS: &str = "j/k:job  enter:runs  /:search  e:edit  d:activity  q:quit  ?:help";

pub fn draw(frame: &mut Frame<'_>, app: &App) {
    if app.job_open.is_some() {
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
    frame.render_widget(Paragraph::new(Line::from(spans)), header);

    draw_job_list(frame, app, &jobs, list);
    draw_config(frame, app.dag_selected_agent(), config);
}

fn draw_job_list(frame: &mut Frame<'_>, app: &App, jobs: &[JobEntry<'_>], area: Rect) {
    let now = chrono::Utc::now();
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
        lines.push(job_line(job, selected, now));

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
            if app.job_search.is_empty() {
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
    now: chrono::DateTime<chrono::Utc>,
) -> Line<'a> {
    let trigger = job
        .spec
        .schedule
        .as_deref()
        .map_or_else(|| "manual".to_owned(), |s| format!("cron {s}"));
    let mut spans = vec![
        Span::styled(
            if job.running.is_empty() {
                "▸ "
            } else {
                "● "
            },
            Style::new().fg(if job.running.is_empty() {
                Color::Reset
            } else {
                Color::Green
            }),
        ),
        Span::styled(
            job.spec.name.clone(),
            Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("  {}  {trigger}", job.spec.runner),
            Style::new().add_modifier(Modifier::DIM),
        ),
    ];
    if !job.spec.enabled {
        spans.push(Span::styled("  disabled", Style::new().fg(Color::DarkGray)));
    }
    // While it runs, WHERE it is matters more than how it last ended.
    if job.running.is_empty() {
        match job.last {
            Some(run) => spans.push(Span::styled(
                format!(
                    "  last: {} · {} ago",
                    run.state,
                    elapsed_between(run.created_at, now)
                ),
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

/// Box-drawing guides for rows given in depth-first order, row 0 being
/// the root (which gets none). `├─` for a node with a later sibling,
/// `└─` for the last, and `│` carried down for every ancestor that still
/// has siblings to come.
pub(crate) fn tree_prefixes(depths: &[usize]) -> Vec<String> {
    let mut out = Vec::with_capacity(depths.len());
    // more[k]: the ancestor at depth k+1 has later siblings, so its
    // column needs a `│` running down past this row.
    let mut more: Vec<bool> = Vec::new();
    for (i, &depth) in depths.iter().enumerate() {
        if depth == 0 {
            more.clear();
            out.push(String::new());
            continue;
        }
        more.truncate(depth - 1);
        let last = depths[i + 1..]
            .iter()
            .find(|&&d| d <= depth)
            .is_none_or(|&d| d < depth);
        let mut prefix = String::new();
        for &open in &more {
            prefix.push_str(if open { "│  " } else { "   " });
        }
        prefix.push_str(if last { "└─ " } else { "├─ " });
        more.push(!last);
        out.push(prefix);
    }
    out
}

/// The slice of `lines` that fits `area`, scrolled just enough to keep
/// `selected` on screen.
pub(crate) fn window(lines: Vec<Line<'_>>, selected: usize, area: Rect) -> Vec<Line<'_>> {
    let visible = usize::from(area.height.saturating_sub(2)).max(1);
    let start = selected.saturating_sub(visible.saturating_sub(2));
    lines.into_iter().skip(start).take(visible).collect()
}

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
            let body = std::fs::read_to_string(&agent.source)
                .unwrap_or_else(|e| format!("cannot read {}: {e}", agent.source.display()));
            (
                format!("CONFIG · {name} · e to edit"),
                body.lines().map(|l| Line::from(l.to_owned())).collect(),
            )
        }
    };
    let mut lines = lines;
    lines.truncate(usize::from(area.height.saturating_sub(2)));
    frame.render_widget(Paragraph::new(lines).block(pane_block(&title)), area);
}

pub(crate) fn badge(text: &str) -> Span<'static> {
    Span::styled(
        text.to_owned(),
        Style::new()
            .fg(Color::Black)
            .bg(Color::Magenta)
            .add_modifier(Modifier::BOLD),
    )
}

pub(crate) fn hints(app: &App, default: &'static str) -> Span<'static> {
    match app.status_message() {
        Some(status) => Span::styled(
            format!(" {status} "),
            Style::new().fg(Color::Magenta).add_modifier(Modifier::BOLD),
        ),
        None => Span::styled(default, Style::new().add_modifier(Modifier::DIM)),
    }
}

pub(crate) fn pane_block(title: &str) -> Block<'static> {
    Block::bordered().title(Span::styled(
        format!(" {title} "),
        Style::new().add_modifier(Modifier::BOLD),
    ))
}

pub(crate) fn state_style(state: RunState) -> Style {
    match state {
        RunState::Running => Style::new().fg(Color::Green),
        RunState::Pending => Style::new().fg(Color::Yellow),
        RunState::Succeeded => Style::new().fg(Color::Blue),
        RunState::Failed => Style::new().fg(Color::Red),
        RunState::TimedOut => Style::new().fg(Color::Magenta),
        RunState::Killed => Style::new().fg(Color::DarkGray),
    }
}

#[cfg(test)]
mod tests {
    use super::tree_prefixes;

    #[test]
    fn guides_follow_the_shape_of_the_tree() {
        // root
        // ├─ a
        // │  └─ b
        // └─ c
        //    └─ d
        assert_eq!(
            tree_prefixes(&[0, 1, 2, 1, 2]),
            vec!["", "├─ ", "│  └─ ", "└─ ", "   └─ "]
        );
    }

    #[test]
    fn a_straight_chain_is_all_last_children() {
        assert_eq!(tree_prefixes(&[0, 1, 2]), vec!["", "└─ ", "   └─ "]);
    }

    #[test]
    fn siblings_under_a_deep_node_keep_the_ancestor_bar() {
        // root
        // ├─ ctx
        // │  ├─ impl
        // │  └─ inv
        // └─ docs
        assert_eq!(
            tree_prefixes(&[0, 1, 2, 2, 1]),
            vec!["", "├─ ", "│  ├─ ", "│  └─ ", "└─ "]
        );
    }
}
