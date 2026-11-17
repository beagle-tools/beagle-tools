//! Builds the content pane's documents for the selected recap: the Summary
//! tab (header + write-up + diagrams) and the Diff tab.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use beagle_tui::doc::{blank, dim, push_diagram, push_warnings};

use crate::diff::{self, RenderedDiff};
use crate::markdown;
use crate::model::PrState;
use crate::store::Recap;
use crate::wrap::DocLine;

/// The colour for a PR state.
#[must_use]
pub fn state_color(state: PrState) -> Color {
    match state {
        PrState::Open => Color::Green,
        PrState::Draft => Color::DarkGray,
        PrState::Merged => Color::Magenta,
        PrState::Closed => Color::Red,
    }
}

fn link(label: &str, key: &str, url: &str) -> DocLine {
    beagle_tui::doc::link(&format!("{label:<6} "), 0, key, url)
}

/// The Summary tab: the PR's header, `summary.md`, then its diagrams.
#[must_use]
pub fn summary(recap: &Recap) -> Vec<DocLine> {
    let m = &recap.manifest;
    let state = recap.state();
    let color = state_color(state);
    let mut meta = vec![
        Span::styled("● ", Style::default().fg(color)),
        Span::styled(
            state.label(),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("  ·  {}#{}", m.repo, m.number), dim()),
    ];
    if let (Some(branch), Some(base)) = (&m.branch, &m.base) {
        meta.push(Span::styled(format!("  ·  {branch} → {base}"), dim()));
    }
    meta.push(Span::styled("  ·  ", dim()));
    meta.push(Span::styled(
        format!("+{}", m.additions),
        Style::default().fg(Color::Green),
    ));
    meta.push(Span::raw(" "));
    meta.push(Span::styled(
        format!("−{}", m.deletions),
        Style::default().fg(Color::Red),
    ));
    meta.push(Span::styled(
        format!(
            " in {} file{}",
            m.files,
            if m.files == 1 { "" } else { "s" }
        ),
        dim(),
    ));
    if let Some(author) = &m.author {
        meta.push(Span::styled(format!("  ·  by {author}"), dim()));
    }

    let mut out = vec![
        DocLine::prose(Line::styled(
            m.title.clone(),
            Style::default().add_modifier(Modifier::BOLD),
        )),
        DocLine::prose(Line::from(meta)),
        link("pr", "o", &m.url),
    ];
    match (&m.ticket, &m.ticket_url) {
        (Some(t), Some(url)) => out.push(link(t, "t", url)),
        (Some(t), None) => out.push(DocLine::pre(Line::styled(
            format!("{t:<7}(no ticket_url in recap.toml)"),
            dim(),
        ))),
        (None, _) => {}
    }
    let files = diff::stats(&recap.diff).len();
    out.push(DocLine::prose(Line::from(vec![
        Span::styled("diff   ", dim()),
        Span::styled(
            format!("{files} file{} changed", if files == 1 { "" } else { "s" }),
            Style::default().fg(Color::Cyan),
        ),
        Span::styled("  [2 to view · n jumps file by file]", dim()),
    ])));
    out.push(blank());
    if recap
        .summary
        .trim()
        .lines()
        .all(|l| l.trim().is_empty() || l.starts_with("## "))
    {
        out.push(DocLine::prose(Line::styled(
            "(summary.md has no write-up yet — the agent hasn't written it. The Diff tab (2) has the change.)",
            dim(),
        )));
        out.push(blank());
    }
    out.extend(markdown::to_lines(&recap.summary));
    for d in &recap.diagrams {
        push_diagram(&mut out, &d.name, &d.body);
    }
    push_warnings(&mut out, None, &recap.warnings);
    out
}

/// The Diff tab.
#[must_use]
pub fn diff(recap: &Recap) -> RenderedDiff {
    diff::render(&recap.diff)
}
