//! The RUN pane: metadata for whichever run the FLOWS cursor is on.
//!
//! Split out of [`crate::dag`] to keep that file inside the 500-line limit.

use agent_core::types::Run;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::format::elapsed_between;

/// Metadata for the run highlighted in FLOWS. Shown instead of the manifest
/// because once the cursor is on a run, the manifest is the thing you already
/// know and the run's own numbers are what you came for.
pub fn run_detail(run: &Run) -> Vec<Line<'static>> {
    let dim = Style::new().add_modifier(Modifier::DIM);
    let mut out: Vec<Line<'static>> = Vec::new();
    let mut row = |k: &str, v: String| {
        out.push(Line::from(vec![
            Span::styled(format!("{k:<12}"), dim),
            Span::raw(v),
        ]));
    };
    row("run", run.id.clone());
    row("agent", format!("{}/{}", run.project, run.agent));
    row("state", run.state.to_string());
    row("trigger", run.trigger.to_string());
    if let Some(m) = &run.model {
        row("model", m.clone());
    }
    row("cost", crate::format::cost(run.cost_usd));
    if let Some(n) = run.num_turns {
        // The runner calls this num_turns, but it counts agent-loop
        // round-trips — model acts, tool runs, result returns — not
        // conversation turns. Measured: 65 reported against 64 tool calls and
        // 115 assistant messages, so this is what it is.
        row("tool calls", n.to_string());
    }
    if let Some(code) = run.exit_code {
        row("exit", code.to_string());
    }
    let started = run.started_at.unwrap_or(run.created_at);
    row(
        "started",
        started
            .with_timezone(&chrono::Local)
            .format("%H:%M:%S")
            .to_string(),
    );
    if let Some(end) = run.finished_at {
        row("duration", elapsed_between(started, end));
    }
    if let Some(parent) = &run.parent_run_id {
        row("parent", parent.clone());
    }
    if let Some(s) = &run.tmux_session {
        row("tmux", s.clone());
    }
    if let Some(w) = &run.workdir {
        row("workdir", w.display().to_string());
    }
    // Params spelled out one per line: `ticket=EI-490` is the single most
    // useful thing on this pane and it is buried inside a JSON blob.
    if let Some(raw) = &run.params {
        if let Ok(serde_json::Value::Object(map)) = serde_json::from_str(raw) {
            out.push(Line::from(""));
            out.push(Line::from(Span::styled("params", dim)));
            for (k, v) in map {
                let shown = v.as_str().map_or_else(|| v.to_string(), str::to_owned);
                out.push(Line::from(format!("  {k} = {shown}")));
            }
        }
    }
    // Where the written output landed, if the stage produced any.
    out.push(Line::from(""));
    out.push(Line::from(vec![
        Span::styled("data dir     ", dim),
        Span::raw(format!("~/.local/state/agentd/data/{}", run.id)),
    ]));
    out
}

/// Something you might want to open from a run: a Linear ticket, a PR, a
/// write-up on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub label: String,
    /// A URL, or an absolute path. `open` handles both on macOS.
    pub target: String,
}

/// The issue key a run was started with, if any (`-p ticket=EI-490`).
pub fn ticket_key(run: &Run) -> Option<String> {
    let json: serde_json::Value = serde_json::from_str(run.params.as_deref()?).ok()?;
    let raw = json.as_object()?.get("ticket")?.as_str()?;
    // Accept a full URL as readily as a key, same as the pipeline does.
    if let Some(from_url) = raw
        .rsplit("/issue/")
        .next()
        .filter(|_| raw.contains("/issue/"))
        .and_then(|rest| rest.split('/').next())
    {
        return Some(from_url.to_owned());
    }
    Some(raw.to_owned())
}

/// Everything openable for this run, best first.
///
/// Only the ticket survives indefinitely — it comes from the run's params,
/// which live on the row. The PR and the write-ups are read out of the run's
/// data dir, which retention deletes with the logs, so they disappear from
/// here once the run ages past the retention window.
/// Links for a run, plus the chain-level write-up if its root has one.
///
/// Takes the app because the chain root is only knowable from the other runs
/// in view — the run itself only knows its parent.
pub fn run_links_in_context(app: &crate::app::App, run: &Run) -> Vec<Link> {
    let mut out = Vec::new();
    // The whole-ticket write-up first: it is the one another agent would be
    // handed to pick the work up, and it outranks any single stage's.
    let root = chain_root(app, run);
    if root != run.id {
        if let Some(p) = summary_path(&root).filter(|p| p.is_file()) {
            out.push(Link {
                label: "open CHAIN summary".to_owned(),
                target: p.display().to_string(),
            });
        }
    }
    out.extend(run_links(run));
    out
}

/// Walk up `parent_run_id` to the root of this run's chain.
fn chain_root(app: &crate::app::App, run: &Run) -> String {
    let by_id: std::collections::HashMap<&str, &Run> = app
        .active
        .iter()
        .chain(app.recent.iter())
        .map(|r| (r.id.as_str(), r))
        .collect();
    let mut cur = run;
    for _ in 0..32 {
        match cur.parent_run_id.as_deref().and_then(|p| by_id.get(p)) {
            Some(parent) => cur = parent,
            None => break,
        }
    }
    cur.id.clone()
}

pub fn run_links(run: &Run) -> Vec<Link> {
    let mut out = Vec::new();
    // First, because it is the thing you actually want to read.
    if let Some(p) = summary_path(&run.id).filter(|p| p.is_file()) {
        out.push(Link {
            label: "open summary".to_owned(),
            target: p.display().to_string(),
        });
    }
    if let Some(key) = ticket_key(run) {
        out.push(Link {
            label: format!("Linear {key}"),
            target: format!("https://linear.app/superblocks/issue/{key}"),
        });
    }
    let data = agent_core::paths::data_dir().ok().map(|d| d.join(&run.id));
    if let Some(dir) = &data {
        let result = dir.join("result.md");
        if let Some(pr) = agent_core::links::first_url_in_file(&result, "/pull/") {
            out.push(Link {
                label: "Draft PR".to_owned(),
                target: pr,
            });
        }
        for (file, label) in [
            ("result.md", "result.md"),
            ("context.md", "context pack"),
            ("doc-findings.md", "doc findings"),
            ("summary.md", "summary.md"),
        ] {
            let p = dir.join(file);
            if p.is_file() {
                out.push(Link {
                    label: label.to_owned(),
                    target: p.display().to_string(),
                });
            }
        }
    }
    // The investigate stage writes outside the data dir precisely so this
    // one outlives retention.
    if let Some(home) = std::env::var_os("HOME") {
        let w = std::path::PathBuf::from(home)
            .join("GitHub/matthewmyrick-superblocks/investigations")
            .join(&run.id)
            .join("OUTPUT.md");
        if w.is_file() {
            out.push(Link {
                label: "investigation OUTPUT.md".to_owned(),
                target: w.display().to_string(),
            });
        }
    }
    out
}

/// Where a run's write-up lives. Deliberately NOT the data dir: that is
/// deleted with the logs at four hours, and a summary is meant to be read
/// the next morning. Swept separately on a 24-hour window.
pub fn summary_path(run_id: &str) -> Option<std::path::PathBuf> {
    Some(
        agent_core::paths::state_dir()
            .ok()?
            .join("summaries")
            .join(format!("{run_id}.md")),
    )
}

/// The run's own one-line headline, written by the agent to notify.txt.
pub fn run_summary(run: &Run) -> Option<String> {
    let dir = agent_core::paths::data_dir().ok()?.join(&run.id);
    let text = std::fs::read_to_string(dir.join("notify.txt")).ok()?;
    let joined = text.split_whitespace().collect::<Vec<_>>().join(" ");
    (!joined.is_empty()).then_some(joined)
}

/// The run whichever screen is showing has selected: the FLOWS cursor on the
/// DAG page, the table cursor on the dashboard.
pub fn focused_run(app: &crate::app::App) -> Option<&Run> {
    if app.show_dag {
        app.dag_selected_run()
    } else {
        app.selected_run()
    }
}

/// The expanded run pane: 85% of the screen, the summary, every field, and a
/// navigable list of things to open.
pub fn draw_overlay(frame: &mut ratatui::Frame<'_>, app: &crate::app::App) {
    use ratatui::layout::{Constraint, Flex, Layout};
    use ratatui::style::Color;
    use ratatui::widgets::{Block, Clear, Paragraph};

    let area = frame.area();
    let [area] = Layout::horizontal([Constraint::Percentage(85)])
        .flex(Flex::Center)
        .areas(area);
    let [area] = Layout::vertical([Constraint::Percentage(85)])
        .flex(Flex::Center)
        .areas(area);

    let Some(run) = focused_run(app) else {
        return;
    };
    let dim = Style::new().add_modifier(Modifier::DIM);
    let mut lines: Vec<Line<'static>> = Vec::new();

    if let Some(summary) = run_summary(run) {
        lines.push(Line::from(Span::styled(
            summary,
            Style::new().fg(Color::Green).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));
    }

    let openable = run_links_in_context(app, run);
    if openable.is_empty() {
        lines.push(Line::from(Span::styled(
            "nothing to open — this run's data dir was pruned, or it had no outputs",
            dim,
        )));
    } else {
        lines.push(Line::from(Span::styled(
            "OPEN  enter/o open · y copy path",
            dim,
        )));
        for (i, link) in openable.iter().enumerate() {
            let marker = if i == app.detail_selected {
                "▸ "
            } else {
                "  "
            };
            let style = if i == app.detail_selected {
                Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::new().fg(Color::Cyan)
            };
            lines.push(Line::from(vec![
                Span::raw(marker),
                Span::styled(format!("{:<26}", link.label), style),
                Span::styled(link.target.clone(), dim),
            ]));
        }
    }
    lines.push(Line::from(""));
    lines.extend(run_detail(run));

    let visible = usize::from(area.height.saturating_sub(2));
    lines.truncate(visible);
    let block = Block::bordered().title(Span::styled(
        format!(
            " RUN · {}/{} · j/k move  enter open  m/esc close ",
            run.project, run.agent
        ),
        Style::new().add_modifier(Modifier::BOLD),
    ));
    frame.render_widget(Clear, area);
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

#[cfg(test)]
// Tests should fail loudly on a broken fixture; unwrapping is the clearest way.
#[allow(clippy::unwrap_used)]
mod tests {
    use agent_core::types::{RunState, Trigger};
    use chrono::Utc;

    use super::*;

    fn run(params: Option<&str>) -> Run {
        Run {
            id: "r1".to_owned(),
            project: "superblocks".to_owned(),
            agent: "linear".to_owned(),
            state: RunState::Succeeded,
            trigger: Trigger::Manual,
            parent_run_id: None,
            params: params.map(str::to_owned),
            tmux_session: None,
            log_path: None,
            workdir: None,
            model: None,
            created_at: Utc::now(),
            started_at: None,
            finished_at: None,
            exit_code: None,
            cost_usd: None,
            num_turns: None,
            usage_json: None,
        }
    }

    #[test]
    fn ticket_key_accepts_a_key_or_a_url() {
        assert_eq!(
            ticket_key(&run(Some(r#"{"ticket":"EI-490"}"#))).as_deref(),
            Some("EI-490")
        );
        // The pipeline accepts a pasted URL, so this has to as well.
        assert_eq!(
            ticket_key(&run(Some(
                r#"{"ticket":"https://linear.app/superblocks/issue/EI-490/some-slug"}"#
            )))
            .as_deref(),
            Some("EI-490")
        );
        assert_eq!(ticket_key(&run(None)), None);
        assert_eq!(ticket_key(&run(Some("not json"))), None);
    }

    #[test]
    fn a_ticket_always_yields_a_linear_link() {
        // The ticket lives on the run row, so this one survives retention
        // even after the data dir holding the PR link is pruned.
        let links = run_links(&run(Some(r#"{"ticket":"EI-490"}"#)));
        assert_eq!(links[0].label, "Linear EI-490");
        assert_eq!(
            links[0].target,
            "https://linear.app/superblocks/issue/EI-490"
        );
        // No params, no data dir: nothing to offer rather than a broken link.
        assert_eq!(run_links(&run(None)), [] as [Link; 0]);
    }

    #[test]
    fn pr_urls_are_recovered_without_trailing_punctuation() {
        let dir = std::env::temp_dir().join(format!("agenttop-detail-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("result.md");
        std::fs::write(
            &f,
            "- **PR:** https://github.com/superblocksteam/dedicated-deployment/pull/1333 (draft)\n",
        )
        .unwrap();
        assert_eq!(
            agent_core::links::first_url_in_file(&f, "/pull/").as_deref(),
            Some("https://github.com/superblocksteam/dedicated-deployment/pull/1333")
        );
        assert_eq!(agent_core::links::first_url_in_file(&f, "/nope/"), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
