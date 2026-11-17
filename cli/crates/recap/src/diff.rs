//! Renders a unified diff (as `gh pr diff` prints it) into tagged document
//! lines: a per-file summary up top, then each file under a header, with
//! added lines green, removed red, hunk headers magenta.
//!
//! Every line is preformatted (never wrapped), so code keeps its shape and
//! the pane pans sideways for long lines. Tabs are expanded to four spaces
//! because terminals and ratatui disagree on their width.
//!
//! When `delta` is installed, [`render_delta`] is used instead: it runs the
//! user's own `delta --side-by-side` (the same pager `git diff` uses) at the
//! pane's width and renders its ANSI output, so the Diff tab looks exactly
//! like their git diffs. [`render`] is the fallback.

use std::io::Write as _;
use std::process::{Command, Stdio};

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use crate::wrap::DocLine;

/// One file's change counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileStat {
    /// The file path (the new path for renames).
    pub path: String,
    /// Lines added.
    pub added: usize,
    /// Lines removed.
    pub removed: usize,
    /// `new`, `deleted`, `renamed from …`, or empty.
    pub note: String,
}

/// A rendered diff.
#[derive(Debug, Clone, Default)]
pub struct RenderedDiff {
    /// The document lines.
    pub lines: Vec<DocLine>,
    /// Index into `lines` of each file's header, in order (for `n`/`N`).
    pub file_starts: Vec<usize>,
    /// Per-file counts.
    pub files: Vec<FileStat>,
}

/// Splits `source` into files and counts each one.
#[must_use]
pub fn stats(source: &str) -> Vec<FileStat> {
    let mut files: Vec<FileStat> = Vec::new();
    for line in source.lines() {
        if let Some(rest) = line.strip_prefix("diff --git ") {
            files.push(FileStat {
                path: path_from_git_header(rest),
                added: 0,
                removed: 0,
                note: String::new(),
            });
            continue;
        }
        let Some(file) = files.last_mut() else {
            continue;
        };
        if line.starts_with("new file mode") {
            "new".clone_into(&mut file.note);
        } else if line.starts_with("deleted file mode") {
            "deleted".clone_into(&mut file.note);
        } else if let Some(from) = line.strip_prefix("rename from ") {
            file.note = format!("renamed from {from}");
        } else if let Some(to) = line.strip_prefix("rename to ") {
            to.clone_into(&mut file.path);
        } else if line.starts_with("+++ ") || line.starts_with("--- ") {
        } else if line.starts_with('+') {
            file.added += 1;
        } else if line.starts_with('-') {
            file.removed += 1;
        }
    }
    files
}

/// `a/path b/path` → `path` (the `b/` side).
fn path_from_git_header(rest: &str) -> String {
    rest.rsplit_once(" b/")
        .map_or_else(|| rest.to_owned(), |(_, b)| b.to_owned())
}

/// Renders `source` for the Diff tab.
#[must_use]
pub fn render(source: &str) -> RenderedDiff {
    let files = stats(source);
    let mut out = RenderedDiff {
        files: files.clone(),
        ..RenderedDiff::default()
    };
    let dim = Style::default().fg(Color::DarkGray);
    if source.trim().is_empty() {
        out.lines.push(DocLine::pre(Line::styled(
            "(no diff — run `beagle recap refresh <id>` to pull it)",
            dim,
        )));
        return out;
    }

    push_file_table(&mut out.lines, &files);

    let mut file_index = 0usize;
    for raw in source.lines() {
        let line = expand_tabs(raw.trim_end_matches('\r'));
        if line.starts_with("diff --git ") {
            let stat = files.get(file_index);
            file_index += 1;
            out.lines.push(DocLine::pre(Line::default()));
            out.file_starts.push(out.lines.len());
            let path = stat.map_or("", |f| f.path.as_str());
            let mut spans = vec![Span::styled(
                format!("━━ {path} "),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )];
            if let Some(f) = stat {
                spans.push(Span::styled(
                    format!("+{} ", f.added),
                    Style::default().fg(Color::Green),
                ));
                spans.push(Span::styled(
                    format!("−{} ", f.removed),
                    Style::default().fg(Color::Red),
                ));
                if !f.note.is_empty() {
                    spans.push(Span::styled(format!("({}) ", f.note), dim));
                }
            }
            spans.push(Span::styled(
                "━".repeat(30),
                Style::default().fg(Color::Cyan),
            ));
            out.lines.push(DocLine::pre(Line::from(spans)));
            continue;
        }
        // Git's own bookkeeping lines carry nothing the header doesn't.
        if [
            "index ",
            "--- ",
            "+++ ",
            "new file mode",
            "deleted file mode",
            "similarity index",
            "rename from ",
            "rename to ",
            "old mode",
            "new mode",
        ]
        .iter()
        .any(|p| line.starts_with(p))
        {
            continue;
        }
        let style = if line.starts_with("@@") {
            Style::default().fg(Color::Magenta)
        } else if line.starts_with('+') {
            Style::default().fg(Color::Green)
        } else if line.starts_with('-') {
            Style::default().fg(Color::Red)
        } else if line.starts_with('\\') || line.starts_with("Binary files") {
            dim
        } else {
            Style::default()
        };
        out.lines.push(DocLine::pre(Line::styled(line, style)));
    }
    out
}

/// The "N files changed" line plus one `+a −r  path` row per file.
fn push_file_table(lines: &mut Vec<DocLine>, files: &[FileStat]) {
    let dim = Style::default().fg(Color::DarkGray);
    let (added, removed): (usize, usize) = files
        .iter()
        .fold((0, 0), |(a, r), f| (a + f.added, r + f.removed));
    lines.push(DocLine::pre(Line::from(vec![
        Span::styled(
            format!("{} files changed  ", files.len()),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("+{added} "), Style::default().fg(Color::Green)),
        Span::styled(format!("−{removed}"), Style::default().fg(Color::Red)),
        Span::styled("    n / N jump between files", dim),
    ])));
    let width = files
        .iter()
        .map(|f| f.added.to_string().len())
        .max()
        .unwrap_or(1);
    let rwidth = files
        .iter()
        .map(|f| f.removed.to_string().len())
        .max()
        .unwrap_or(1);
    for f in files {
        let mut spans = vec![
            Span::styled(
                format!("  +{:<width$} ", f.added),
                Style::default().fg(Color::Green),
            ),
            Span::styled(
                format!("−{:<rwidth$}  ", f.removed),
                Style::default().fg(Color::Red),
            ),
            Span::raw(f.path.clone()),
        ];
        if !f.note.is_empty() {
            spans.push(Span::styled(format!("  ({})", f.note), dim));
        }
        lines.push(DocLine::pre(Line::from(spans)));
    }
}

/// Whether the `delta` pager is installed and runnable.
#[must_use]
pub fn delta_available() -> bool {
    Command::new("delta")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Runs `delta --side-by-side` on `source` at `width` columns and returns its
/// raw ANSI output. `None` if delta is missing or fails.
fn run_delta(source: &str, width: u16) -> Option<String> {
    let mut child = Command::new("delta")
        .args(["--side-by-side", "--paging=never", "--width"])
        .arg(width.max(40).to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    // Feed stdin from a thread so a large diff can't deadlock against a
    // full stdout pipe.
    let mut stdin = child.stdin.take()?;
    let input = source.to_owned();
    let writer = std::thread::spawn(move || {
        let _ = stdin.write_all(input.as_bytes()); // delta exiting early is reported below
    });
    let out = child.wait_with_output().ok()?;
    let _ = writer.join();
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The Diff tab rendered by `delta` (side by side, the user's git look),
/// with the per-file table above it. `None` when delta isn't usable, so the
/// caller can fall back to [`render`].
#[must_use]
pub fn render_delta(source: &str, width: u16) -> Option<RenderedDiff> {
    if source.trim().is_empty() {
        return Some(render(source));
    }
    let output = run_delta(source, width)?;
    Some(from_delta_output(source, &output))
}

/// Builds the Diff tab from delta's ANSI `output` for the diff `source`.
/// File headers are found by delta's own layout: the file's path on one
/// line, a `───` rule on the next.
#[must_use]
pub fn from_delta_output(source: &str, output: &str) -> RenderedDiff {
    let files = stats(source);
    let mut out = RenderedDiff {
        files: files.clone(),
        ..RenderedDiff::default()
    };
    push_file_table(&mut out.lines, &files);
    out.lines.push(DocLine::pre(Line::default()));
    let offset = out.lines.len();

    let text = crate::ansi::to_text(output);
    let plain: Vec<String> = crate::ansi::strip(output)
        .lines()
        .map(str::to_owned)
        .collect();
    let mut next_file = 0usize;
    for (i, line) in plain.iter().enumerate() {
        let Some(f) = files.get(next_file) else { break };
        let is_rule = plain
            .get(i + 1)
            .is_some_and(|n| n.trim_start().starts_with('─'));
        if is_rule && line.trim().ends_with(f.path.as_str()) {
            out.file_starts.push(offset + i);
            next_file += 1;
        }
    }
    out.lines.extend(text.lines.into_iter().map(DocLine::pre));
    out
}

fn expand_tabs(s: &str) -> String {
    if s.contains('\t') {
        s.replace('\t', "    ")
    } else {
        s.to_owned()
    }
}

#[cfg(test)]
#[path = "tests/diff.rs"]
mod tests;
