//! `beagle recap show`: a recap as one markdown document.

use std::fmt::Write as _;

use super::Recap;
use crate::ansi;

/// Renders a recap as one markdown document (for agents and pasting): the
/// metadata, the summary, the diagrams, and the per-file stat. The full diff
/// is appended only when `with_diff` is set.
#[must_use]
// `write!` into a `String` is infallible, so its `fmt::Result`s are dropped.
pub fn export_markdown(recap: &Recap, with_diff: bool) -> String {
    let m = &recap.manifest;
    let mut out = String::new();
    let _ = write!(out, "# {}#{} — {}\n\n", m.repo, m.number, m.title);
    let _ = writeln!(out, "- state: {}", m.state);
    let _ = writeln!(out, "- pr: {}", m.url);
    if let Some(t) = &m.ticket {
        let _ = writeln!(
            out,
            "- ticket: {t}{}",
            m.ticket_url
                .as_ref()
                .map(|u| format!(" ({u})"))
                .unwrap_or_default()
        );
    }
    if let (Some(b), Some(base)) = (&m.branch, &m.base) {
        let _ = writeln!(out, "- branch: {b} → {base}");
    }
    let _ = write!(
        out,
        "- size: +{} −{} in {} files\n\n",
        m.additions, m.deletions, m.files
    );
    out.push_str(recap.summary.trim_end());
    out.push_str("\n\n");
    for d in &recap.diagrams {
        let _ = write!(
            out,
            "```text\n# {}\n{}\n```\n\n",
            d.name,
            ansi::strip(d.body.trim_end())
        );
    }
    out.push_str("## Files changed\n\n```text\n");
    for f in crate::diff::stats(&recap.diff) {
        let _ = writeln!(
            out,
            "+{:<5} -{:<5} {}{}",
            f.added,
            f.removed,
            f.path,
            if f.note.is_empty() {
                String::new()
            } else {
                format!("  ({})", f.note)
            }
        );
    }
    out.push_str("```\n");
    if with_diff {
        let _ = write!(
            out,
            "\n## Diff\n\n```diff\n{}\n```\n",
            recap.diff.trim_end()
        );
    }
    out
}
