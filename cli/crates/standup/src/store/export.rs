//! `beagle standup show`: a whole report as one markdown document.

use std::fmt::Write as _;

use super::{Diagram, Report};
use crate::ansi;

/// Renders a whole report as one markdown document (for agents, pasting,
/// or archiving). ANSI codes are stripped from diagrams.
#[must_use]
// `write!` into a `String` is infallible, so its `fmt::Result`s are dropped.
pub fn export_markdown(report: &Report) -> String {
    let mut out = String::new();
    let heading = report.manifest.title.as_deref().map_or_else(
        || format!("Standup {}", report.date),
        |t| format!("Standup {} — {t}", report.date),
    );
    let _ = write!(out, "# {heading}\n\n");
    if let Some(since) = &report.manifest.since {
        let _ = write!(out, "Covers work since {since}.\n\n");
    }
    out.push_str("## Overview\n\n");
    out.push_str(report.overview.trim_end());
    out.push_str("\n\n");
    push_diagrams(&mut out, &report.diagrams);
    for t in &report.tickets {
        let m = &t.manifest;
        let _ = write!(out, "---\n\n## {} — {}\n\n", t.id, m.title);
        if let Some(state) = &m.state {
            let _ = writeln!(out, "- state: {state}");
        }
        if let Some(url) = &m.url {
            let _ = writeln!(out, "- ticket: {url}");
        }
        if !m.systems.is_empty() {
            let _ = writeln!(out, "- systems: {}", m.systems.join(", "));
        }
        for pr in &m.prs {
            let _ = writeln!(out, "- pr: {pr}");
        }
        out.push('\n');
        out.push_str(t.summary.trim_end());
        out.push_str("\n\n");
        push_diagrams(&mut out, &t.diagrams);
    }
    out
}

fn push_diagrams(out: &mut String, diagrams: &[Diagram]) {
    for d in diagrams {
        let _ = write!(
            out,
            "```text\n# {}\n{}\n```\n\n",
            d.name,
            ansi::strip(d.body.trim_end())
        );
    }
}
