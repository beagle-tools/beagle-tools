//! The session header and one block per transcript entry.

use super::{
    clock, counts, day_clock, dim, fg, is_lookup, markdown, scout_call, Color, Entry, FileChange,
    HashSet, Line, Modifier, Renderer, Span, Style, SUMMARY_FILES,
};

impl Renderer<'_> {
    /// The header: when, how much, and what it made.
    pub(super) fn summary(&mut self, id: &str, entries: &[Entry]) {
        let short = id.get(..8).unwrap_or(id);
        let span = match (entries.first(), entries.last()) {
            (Some(first), Some(last)) => {
                let (from, to) = (first.ts(), last.ts());
                if from.get(..10) == to.get(..10) {
                    format!(" · {} → {}", clock(from), clock(to))
                } else {
                    format!(" · {} → {}", day_clock(from), day_clock(to))
                }
            }
            _ => String::new(),
        };
        self.lines.push(Line::styled(
            format!("━━ session {short}{span} ━━"),
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::BOLD),
        ));
        let mut prompts = 0;
        let mut commands = 0;
        let mut failed = 0;
        let mut bash = HashSet::new();
        let mut files: Vec<FileChange> = Vec::new();
        for entry in entries {
            match entry {
                Entry::Prompt { .. } => prompts += 1,
                Entry::Tool {
                    id, name, summary, ..
                } if name == "Bash" && scout_call(summary).is_none() => {
                    commands += 1;
                    bash.insert(id.as_str());
                }
                Entry::Result {
                    id, error: true, ..
                } if bash.contains(id.as_str()) => failed += 1,
                Entry::Change {
                    path,
                    created,
                    added,
                    removed,
                    ..
                } => {
                    if let Some(f) = files.iter_mut().find(|f| &f.path == path) {
                        f.added += added;
                        f.removed += removed;
                    } else {
                        files.push(FileChange {
                            path: path.clone(),
                            created: *created,
                            added: *added,
                            removed: *removed,
                        });
                    }
                }
                _ => {}
            }
        }
        let failed = if failed > 0 {
            format!(" ({failed} failed)")
        } else {
            String::new()
        };
        self.lines.push(Line::styled(
            format!(
                "{prompts} prompt(s) · {commands} command(s){failed} · {} file(s) changed",
                files.len()
            ),
            dim(),
        ));
        self.file_list(&files);
        self.lines.push(Line::styled("── timeline ──", dim()));
        if entries.is_empty() {
            self.lines.push(Line::styled(
                "  (nothing recorded yet — it fills in as the agent works)",
                dim().add_modifier(Modifier::ITALIC),
            ));
        }
    }

    /// The summary's list of changed files, aligned, with line counts.
    pub(super) fn file_list(&mut self, files: &[FileChange]) {
        let width = files
            .iter()
            .take(SUMMARY_FILES)
            .map(|f| self.rel(&f.path).chars().count())
            .max()
            .unwrap_or(0);
        for f in files.iter().take(SUMMARY_FILES) {
            let (glyph, color) = if f.created {
                ("✚", Color::Green)
            } else {
                ("✎", Color::Yellow)
            };
            let path = self.rel(&f.path);
            self.lines.push(Line::from(vec![
                Span::styled(
                    format!("  {glyph} "),
                    fg(color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!("{path:<width$}"), fg(color)),
                Span::raw("  "),
                Span::styled(counts(f.added, f.removed), dim()),
            ]));
        }
        if files.len() > SUMMARY_FILES {
            self.lines.push(Line::styled(
                format!("  … and {} more", files.len() - SUMMARY_FILES),
                dim(),
            ));
        }
    }

    pub(super) fn entry(&mut self, entry: &Entry) {
        // Reads and their results are folded into one line; anything else
        // ends the run of reads.
        let is_read = match entry {
            Entry::Tool { name, .. } => is_lookup(name),
            Entry::Result { id, .. } => self.tools.get(id).is_some_and(|n| is_lookup(n)),
            _ => false,
        };
        if !is_read {
            self.flush_reads();
        }
        match entry {
            Entry::Prompt { ts, text, .. } => {
                self.gap();
                self.lines.push(Line::from(vec![
                    Span::styled("› You", fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                    Span::styled(format!(" · {}", clock(ts)), dim()),
                ]));
                for line in text.lines() {
                    self.lines
                        .push(Line::styled(format!("  {line}"), fg(Color::Cyan)));
                }
            }
            Entry::Thinking { text, ms, .. } => self.thinking(text.as_deref(), *ms),
            Entry::Text { text, .. } => {
                self.gap();
                for (i, line) in markdown::to_text(text).lines.into_iter().enumerate() {
                    let lead = if i == 0 {
                        Span::styled("● ", Style::default().add_modifier(Modifier::BOLD))
                    } else {
                        Span::raw("  ")
                    };
                    let mut spans = vec![lead];
                    spans.extend(line.spans);
                    self.lines.push(Line::from(spans));
                }
            }
            Entry::Tool {
                id,
                name,
                summary,
                detail,
                ..
            } => {
                self.tools.insert(id.clone(), name.clone());
                self.tool(id, name, summary, detail.as_deref());
            }
            Entry::Result {
                id, text, error, ..
            } => self.result(id, text, *error),
            Entry::Change {
                path,
                created,
                added,
                removed,
                ..
            } => {
                let (glyph, verb, color) = if *created {
                    ("✚", "created", Color::Green)
                } else {
                    ("✎", "edited ", Color::Yellow)
                };
                self.lines.push(Line::from(vec![
                    Span::styled(
                        format!("{glyph} {verb} "),
                        fg(color).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(self.rel(path), fg(color)),
                    Span::styled(format!("  {}", counts(*added, *removed)), dim()),
                ]));
            }
            Entry::Notice { text, .. } => {
                self.lines.push(Line::styled(format!("◦ {text}"), dim()));
            }
        }
    }
}
