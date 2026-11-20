//! The Diff tab. When `delta` is installed it renders the PR with the
//! user's own `delta --side-by-side` (what `git diff` shows them) at the
//! pane's width; otherwise a plain colored unified diff. Rendering runs
//! once per (PR, commit, width) and is cached — scrolling only slices.
//! The delta approach is the recap TUI's.

use ratatui::style::{Color, Style};
use ratatui::text::Line;

use beagle_tui::ansi;

/// A rendered diff: the lines, and where each file starts in them.
#[derive(Debug, Clone, Default)]
pub struct Rendered {
    pub lines: Vec<Line<'static>>,
    pub files: Vec<(String, usize)>,
}

impl Rendered {
    /// The line a file's section starts on.
    pub fn file_start(&self, path: &str) -> Option<usize> {
        self.files
            .iter()
            .find(|(p, _)| p == path)
            .map(|(_, at)| *at)
    }
}

#[derive(Debug, Default)]
pub struct DiffCache {
    key: Option<(String, String, u16)>,
    rendered: Rendered,
}

impl DiffCache {
    /// The diff for `key` (PR label, commit) at `width`, rendering it
    /// only when any of the three changed.
    pub fn get(&mut self, pr: &str, head: &str, raw: &str, width: u16) -> &Rendered {
        let key = (pr.to_owned(), head.to_owned(), width);
        if self.key.as_ref() != Some(&key) {
            self.rendered = render_delta(raw, width).unwrap_or_else(|| render_plain(raw));
            self.key = Some(key);
        }
        &self.rendered
    }

    /// The last render, whatever its width (for key handling between
    /// draws).
    pub fn last(&self) -> &Rendered {
        &self.rendered
    }
}

/// Paths in the order the diff lists them.
fn paths(raw: &str) -> Vec<String> {
    raw.lines()
        .filter_map(|l| l.strip_prefix("diff --git "))
        .filter_map(|rest| rest.rsplit_once(" b/").map(|(_, p)| p.to_owned()))
        .collect()
}

fn render_delta(raw: &str, width: u16) -> Option<Rendered> {
    if raw.trim().is_empty() {
        return None;
    }
    let output = beagle_tui::delta::side_by_side(raw, width)?;
    let plain: Vec<String> = ansi::strip(&output).lines().map(str::to_owned).collect();
    let text = ansi::to_text(&output);
    let mut files = Vec::new();
    let mut want = paths(raw).into_iter().peekable();
    for (i, line) in plain.iter().enumerate() {
        let Some(path) = want.peek() else { break };
        // delta prints the path, then a `───` rule under it.
        let ruled = plain
            .get(i + 1)
            .is_some_and(|n| n.trim_start().starts_with('─'));
        if ruled && line.trim().ends_with(path.as_str()) {
            files.push((path.clone(), i));
            want.next();
        }
    }
    Some(Rendered {
        lines: text.lines,
        files,
    })
}

/// The fallback: a unified diff, added green, removed red, hunks magenta.
fn render_plain(raw: &str) -> Rendered {
    let mut out = Rendered::default();
    let mut want = paths(raw).into_iter();
    for line in raw.lines() {
        let style = if line.starts_with("diff --git ") {
            if let Some(p) = want.next() {
                out.files.push((p, out.lines.len()));
            }
            Style::default().fg(Color::Cyan)
        } else if line.starts_with("@@") {
            Style::default().fg(Color::Magenta)
        } else if line.starts_with('+') && !line.starts_with("+++") {
            Style::default().fg(Color::Green)
        } else if line.starts_with('-') && !line.starts_with("---") {
            Style::default().fg(Color::Red)
        } else {
            Style::default()
        };
        out.lines
            .push(Line::styled(line.replace('\t', "    "), style));
    }
    out
}

#[cfg(test)]
// Tests panic on a broken fixture; that is the failure mode we want.
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    const RAW: &str = "diff --git a/x.rs b/x.rs\n--- a/x.rs\n+++ b/x.rs\n@@ -1 +1 @@\n-a\n+b\n\
                       diff --git a/y.rs b/y.rs\n--- a/y.rs\n+++ b/y.rs\n@@ -1 +1 @@\n-c\n+d\n";

    #[test]
    fn the_plain_render_finds_every_file() {
        let r = render_plain(RAW);
        assert_eq!(r.file_start("x.rs"), Some(0));
        assert_eq!(r.file_start("y.rs"), Some(6));
        assert_eq!(r.lines.len(), 12);
    }

    #[test]
    fn the_cache_renders_once_per_key() {
        let mut cache = DiffCache::default();
        let n = cache.get("o/r#1", "sha", RAW, 100).lines.len();
        assert!(n > 0);
        assert_eq!(cache.last().lines.len(), n);
        assert_eq!(paths(RAW), vec!["x.rs", "y.rs"]);
    }
}
