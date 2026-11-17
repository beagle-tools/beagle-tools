//! Builds the content pane's document for the selected item: the report
//! overview, or one ticket.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use beagle_tui::doc::{blank, dim, link, push_diagram, push_warnings, rule};

use crate::markdown;
use crate::model::StateKind;
use crate::store::{Diagram, Report, Ticket};
use crate::wrap::DocLine;

/// The colour for a state bucket.
#[must_use]
pub fn state_color(kind: StateKind) -> Color {
    match kind {
        StateKind::Progress => Color::Yellow,
        StateKind::Review => Color::Magenta,
        StateKind::Blocked => Color::Red,
        StateKind::Done => Color::Green,
        StateKind::Todo => Color::Blue,
        StateKind::Other => Color::DarkGray,
    }
}

fn push_diagrams(out: &mut Vec<DocLine>, diagrams: &[Diagram]) {
    for d in diagrams {
        push_diagram(out, &d.name, &d.body);
    }
}

fn push_body(out: &mut Vec<DocLine>, source: &str, empty_hint: &str) {
    if source.trim().is_empty() {
        out.push(DocLine::prose(Line::styled(empty_hint.to_owned(), dim())));
    } else {
        out.extend(markdown::to_lines(source));
    }
}

/// `(label, count)` per state bucket that has tickets, in display order.
#[must_use]
pub fn state_counts(report: &Report) -> Vec<(String, StateKind, usize)> {
    let mut counts: Vec<(String, StateKind, usize)> = Vec::new();
    for t in &report.tickets {
        let label = t
            .manifest
            .state
            .clone()
            .unwrap_or_else(|| "no state".to_owned());
        match counts.iter_mut().find(|(l, _, _)| *l == label) {
            Some(entry) => entry.2 += 1,
            None => counts.push((label, t.state_kind(), 1)),
        }
    }
    counts
}

/// The overview document: headline, overview.md, diagrams, then every
/// ticket at a glance.
#[must_use]
pub fn overview(report: &Report) -> Vec<DocLine> {
    let mut out = Vec::new();
    let mut head = vec![Span::styled(
        format!("Standup · {} {}", report.date.weekday(), report.date),
        Style::default().add_modifier(Modifier::BOLD),
    )];
    if let Some(since) = &report.manifest.since {
        head.push(Span::styled(format!("  ·  since {since}"), dim()));
    }
    out.push(DocLine::prose(Line::from(head)));
    if let Some(title) = &report.manifest.title {
        out.push(DocLine::prose(Line::styled(
            title.clone(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::ITALIC),
        )));
    }
    let mut counts = vec![Span::styled(
        format!("{} tickets", report.tickets.len()),
        dim(),
    )];
    for (label, kind, n) in state_counts(report) {
        counts.push(Span::styled("  ·  ", dim()));
        counts.push(Span::styled(
            format!("{n} {label}"),
            Style::default().fg(state_color(kind)),
        ));
    }
    out.push(DocLine::prose(Line::from(counts)));
    out.push(blank());
    push_body(
        &mut out,
        &report.overview,
        "(overview.md is empty — the agent hasn't written it yet)",
    );
    push_diagrams(&mut out, &report.diagrams);

    if !report.tickets.is_empty() {
        out.push(blank());
        out.push(rule("tickets at a glance"));
        for t in &report.tickets {
            let color = state_color(t.state_kind());
            out.push(DocLine::prose(Line::from(vec![
                Span::styled("● ", Style::default().fg(color)),
                Span::styled(
                    format!("{:<8} ", t.id),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{:<12} ", t.manifest.state.as_deref().unwrap_or("—")),
                    Style::default().fg(color),
                ),
                Span::raw(t.manifest.title.clone()),
            ])));
            if let Some(tldr) = tldr(&t.summary) {
                out.push(DocLine::prose(Line::styled(format!("  {tldr}"), dim())));
            }
        }
    }
    push_warnings(&mut out, Some("load warnings"), &report.warnings);
    out
}

/// The first prose line under a `## TL;DR` heading, if the ticket has one.
#[must_use]
pub fn tldr(summary: &str) -> Option<String> {
    let mut lines = summary.lines();
    lines.find(|l| {
        let l = l.trim().to_ascii_lowercase();
        l.starts_with('#') && l.contains("tl;dr")
    })?;
    lines
        .map(str::trim)
        .take_while(|l| !l.starts_with('#'))
        .find(|l| !l.is_empty())
        .map(|l| {
            l.trim_start_matches("- ")
                .replace("**", "")
                .replace('`', "")
        })
}

/// One ticket's document: header with state, links, and systems, then
/// summary.md, then diagrams.
#[must_use]
pub fn ticket(t: &Ticket) -> Vec<DocLine> {
    let m = &t.manifest;
    let color = state_color(t.state_kind());
    let mut out = vec![
        // The pane's border title already shows the id.
        DocLine::prose(Line::styled(
            m.title.clone(),
            Style::default().add_modifier(Modifier::BOLD),
        )),
        DocLine::prose(Line::from(vec![
            Span::styled("● ", Style::default().fg(color)),
            Span::styled(
                m.state.clone().unwrap_or_else(|| "no state".to_owned()),
                Style::default().fg(color),
            ),
            Span::styled(
                if m.systems.is_empty() {
                    String::new()
                } else {
                    format!("  ·  {}", m.systems.join(", "))
                },
                dim(),
            ),
        ])),
    ];
    match &m.url {
        Some(url) => out.push(link("ticket", 7, "o", url)),
        None => out.push(DocLine::prose(Line::styled(
            "ticket (no url in ticket.toml)",
            dim(),
        ))),
    }
    for pr in &m.prs {
        out.push(link("pr", 7, "p", pr));
    }
    out.push(blank());
    push_body(
        &mut out,
        &t.summary,
        "(summary.md is empty — the agent hasn't written it yet)",
    );
    push_diagrams(&mut out, &t.diagrams);
    out
}

#[cfg(test)]
#[path = "../tests/doc.rs"]
mod tests;
