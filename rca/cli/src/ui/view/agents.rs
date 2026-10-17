//! The agents screen: one block per configured `beagle-agentd` agent with
//! its run state, last tick, and recent results.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::ui::App;

use super::style::pane_block;

/// Draws the agents screen: the live daemon health and per-agent status fed by
/// the background poller. Read-only for now; controls arrive in a later issue.
pub(super) fn draw_agents(frame: &mut Frame, app: &App, area: Rect) {
    use crate::agentd::AgentsStatus;

    let mut lines: Vec<Line> = vec![Line::from("")];
    match app.agents_status() {
        AgentsStatus::Connecting => {
            lines.push(Line::styled(
                "  connecting to beagle-agentd…",
                Style::default().fg(Color::DarkGray),
            ));
        }
        AgentsStatus::Offline(reason) => {
            lines.push(Line::styled(
                "  ● daemon offline",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ));
            lines.push(Line::from(""));
            lines.push(Line::styled(
                format!("  {reason}"),
                Style::default().fg(Color::DarkGray),
            ));
            lines.push(Line::from(""));
            lines.push(Line::styled(
                "  start it with `beagle agent start`",
                Style::default().fg(Color::DarkGray),
            ));
        }
        AgentsStatus::Connected(snapshot) => {
            lines.push(Line::styled(
                format!("  ● beagle-agentd {}", snapshot.version),
                Style::default()
                    .fg(Color::LightGreen)
                    .add_modifier(Modifier::BOLD),
            ));
            lines.push(Line::from(""));
            if snapshot.agents.is_empty() {
                lines.push(Line::styled(
                    "  (no agents configured)",
                    Style::default().fg(Color::DarkGray),
                ));
            }
            for (index, agent) in snapshot.agents.iter().enumerate() {
                lines.push(agent_header_line(agent, index == app.agents_selected()));
                for result in &agent.last_results {
                    lines.push(Line::styled(
                        format!("      {result}"),
                        Style::default().fg(Color::Gray),
                    ));
                }
            }
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::styled(
        "  j/k select · x start/stop · enter log · e edit config · A/esc back",
        Style::default().fg(Color::DarkGray),
    ));
    let block = pane_block(" agents ".to_owned(), false);
    frame.render_widget(Paragraph::new(Text::from(lines)).block(block), area);
}

/// The one-line header for an agent: id, running/paused, sessions, last tick.
/// The selected row is marked and reverse-highlighted.
pub(super) fn agent_header_line(
    agent: &crate::agentd::AgentSnapshot,
    selected: bool,
) -> Line<'static> {
    let (state, color) = if agent.running {
        ("running", Color::LightGreen)
    } else {
        ("paused", Color::Yellow)
    };
    let marker = if selected { "▸ " } else { "  " };
    let id_style = if selected {
        Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED)
    } else {
        Style::default().add_modifier(Modifier::BOLD)
    };
    let mut spans = vec![
        Span::styled(format!("{marker}{}  ", agent.id), id_style),
        Span::styled(format!("[{state}]"), Style::default().fg(color)),
    ];
    if agent.active_sessions > 0 {
        spans.push(Span::styled(
            format!("  {} session(s)", agent.active_sessions),
            Style::default().fg(Color::LightBlue),
        ));
    }
    if let Some(tick) = &agent.last_tick {
        spans.push(Span::styled(
            format!("  · last tick {tick}"),
            Style::default().fg(Color::DarkGray),
        ));
    }
    Line::from(spans)
}
