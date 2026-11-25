//! Thinking, tool calls, commands and their output, and scout banners.

use super::*;

impl Renderer<'_> {
    pub(super) fn thinking(&mut self, text: Option<&str>, ms: Option<u64>) {
        let style = fg(Color::Magenta).add_modifier(Modifier::ITALIC);
        match (text, ms) {
            (Some(text), _) => {
                self.lines.push(Line::styled(
                    "∴ thinking",
                    style.add_modifier(Modifier::DIM),
                ));
                for line in text.lines() {
                    self.lines.push(Line::styled(format!("  {line}"), style));
                }
            }
            (None, Some(ms)) => self.lines.push(Line::styled(
                format!("∴ thought for {}", seconds(ms)),
                style.add_modifier(Modifier::DIM),
            )),
            (None, None) => {}
        }
    }

    pub(super) fn tool(&mut self, id: &str, name: &str, summary: &str, detail: Option<&str>) {
        match name {
            "Bash" => self.command(id, summary, detail),
            // Shown by their Change entry (with line counts) instead.
            "Edit" | "Write" | "NotebookEdit" => {}
            n if is_lookup(n) => {
                let what = if name == "Read" {
                    file_name(summary)
                } else {
                    format!("“{}”", one_line(summary, 40))
                };
                self.reads.push(what);
            }
            "Agent" | "Task" => self.lines.push(Line::from(vec![
                Span::styled("⇢ agent ", fg(Color::Magenta).add_modifier(Modifier::BOLD)),
                Span::styled(detail.map(|d| format!("({d}) ")).unwrap_or_default(), dim()),
                Span::styled(summary.to_owned(), fg(Color::Magenta)),
            ])),
            "Skill" => self.lines.push(Line::styled(
                format!("★ {summary}"),
                fg(Color::Magenta).add_modifier(Modifier::BOLD),
            )),
            "AskUserQuestion" => {
                self.gap();
                self.lines.push(Line::styled(
                    format!("? {summary}"),
                    fg(Color::Cyan).add_modifier(Modifier::BOLD),
                ));
            }
            _ => self.lines.push(Line::styled(
                format!("⚙ {name} {}", one_line(summary, 80)),
                dim(),
            )),
        }
    }

    /// A command: what it is for as the headline (Claude's description),
    /// the command itself on one dim line.
    pub(super) fn command(&mut self, id: &str, summary: &str, detail: Option<&str>) {
        if let Some(banner) = self.banner(summary) {
            self.quiet.insert(id.to_owned());
            self.gap();
            self.lines.push(banner);
            return;
        }
        self.gap();
        let command = one_line(summary, COMMAND_WIDTH);
        if let Some(detail) = detail {
            self.lines.push(Line::styled(
                format!("▸ {detail}"),
                fg(Color::Green).add_modifier(Modifier::BOLD),
            ));
            self.lines.push(Line::from(vec![
                Span::styled("  $ ", dim()),
                Span::styled(command, dim()),
            ]));
        } else {
            self.lines.push(Line::from(vec![
                Span::styled("$ ", fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::styled(command, fg(Color::Green).add_modifier(Modifier::BOLD)),
            ]));
        }
    }

    pub(super) fn result(&mut self, id: &str, text: &str, error: bool) {
        let name = self.tools.get(id).cloned().unwrap_or_default();
        match name.as_str() {
            "Bash" if self.quiet.contains(id) && !error => {}
            "Bash" => self.output(text, error),
            "AskUserQuestion" => {
                for line in text.lines() {
                    self.lines
                        .push(Line::styled(format!("  → {line}"), fg(Color::Cyan)));
                }
            }
            _ if error => {
                let first = text.lines().next().unwrap_or("failed");
                self.lines.push(Line::styled(
                    format!("  ✖ {name}: {}", one_line(first, 100)),
                    fg(Color::LightRed),
                ));
            }
            _ => {}
        }
    }

    /// A command's output: at most [`MAX_OUTPUT`] lines (head and tail),
    /// red with a `✖ exit N` line when it failed.
    pub(super) fn output(&mut self, text: &str, error: bool) {
        let mut lines: Vec<&str> = text.trim_end().lines().collect();
        let status = if error {
            let code = lines
                .first()
                .and_then(|l| l.strip_prefix("Exit code "))
                .map(str::to_owned);
            if code.is_some() {
                lines.remove(0);
            }
            Some(code.map_or_else(|| "failed".to_owned(), |c| format!("exit {c}")))
        } else {
            None
        };
        let color = if error { Color::LightRed } else { Color::Gray };
        let shown: Vec<String> = if lines.len() <= MAX_OUTPUT {
            lines.iter().map(|l| (*l).to_owned()).collect()
        } else {
            let tail = MAX_OUTPUT - HEAD;
            let mut shown: Vec<String> = lines[..HEAD].iter().map(|l| (*l).to_owned()).collect();
            shown.push(format!("⋮ {} more lines", lines.len() - MAX_OUTPUT));
            shown.extend(lines[lines.len() - tail..].iter().map(|l| (*l).to_owned()));
            shown
        };
        for line in shown {
            let style = if line.starts_with('⋮') {
                dim()
            } else {
                fg(color)
            };
            self.lines.push(Line::from(vec![
                Span::styled("  │ ", dim()),
                Span::styled(one_line(&line, 160), style),
            ]));
        }
        if let Some(status) = status {
            self.lines.push(Line::styled(
                format!("  ✖ {status}"),
                fg(Color::LightRed).add_modifier(Modifier::BOLD),
            ));
        }
    }

    pub(super) fn flush_reads(&mut self) {
        if self.reads.is_empty() {
            return;
        }
        let reads = std::mem::take(&mut self.reads);
        let mut list = reads.iter().take(6).cloned().collect::<Vec<_>>().join(", ");
        if reads.len() > 6 {
            let _ = write!(list, " +{} more", reads.len() - 6);
        }
        self.lines.push(Line::from(vec![
            Span::styled("▸ looked at ", fg(Color::Blue)),
            Span::styled(list, fg(Color::Blue).add_modifier(Modifier::DIM)),
        ]));
    }

    /// A blank line before a new block, unless one is already there.
    pub(super) fn gap(&mut self) {
        if self.lines.last().is_some_and(|l| l.width() > 0) {
            self.lines.push(Line::default());
        }
    }

    pub(super) fn banner(&self, command: &str) -> Option<Line<'static>> {
        match scout_call(command)? {
            ScoutCall::Task {
                project,
                task,
                status,
            } => {
                let title = self
                    .store
                    .project(&project)
                    .ok()
                    .and_then(|p| {
                        p.resolve_task(&task)
                            .map(|t| format!("{} · {}", t.id, t.meta.title))
                    })
                    .unwrap_or(task);
                let (text, color) = match status {
                    TaskStatus::Working => (format!("▶ task {title}"), ORANGE),
                    TaskStatus::Paused => (format!("‖ task paused · {title}"), Color::Yellow),
                    TaskStatus::Done => (format!("✔ task done · {title}"), Color::Green),
                    TaskStatus::Waiting => (format!("◷ task waiting · {title}"), Color::LightBlue),
                    TaskStatus::Blocked => (format!("■ task blocked · {title}"), Color::LightRed),
                    TaskStatus::Skipped => (format!("– task skipped · {title}"), Color::DarkGray),
                    TaskStatus::Todo => (format!("○ task back to todo · {title}"), Color::DarkGray),
                };
                Some(Line::styled(text, fg(color).add_modifier(Modifier::BOLD)))
            }
            ScoutCall::Attach { project, task } => {
                let title = self
                    .store
                    .project(&project)
                    .ok()
                    .and_then(|p| p.resolve_task(&task).map(|t| t.id.clone()))
                    .unwrap_or(task);
                Some(Line::styled(
                    format!("⚓ session attached to {title}"),
                    fg(ORANGE),
                ))
            }
            ScoutCall::Log { message, .. } => Some(Line::styled(
                format!("✎ {message}"),
                fg(Color::Cyan).add_modifier(Modifier::ITALIC),
            )),
        }
    }

    /// A short, readable path: relative to the session's directory; else
    /// `repo/path` inside the git repository it belongs to; else `~/…`.
    pub(super) fn rel(&self, path: &str) -> String {
        if let Some(rest) = self
            .cwd
            .as_deref()
            .and_then(|cwd| path.strip_prefix(cwd))
            .and_then(|rest| rest.strip_prefix('/'))
        {
            return rest.to_owned();
        }
        if let Some(short) = repo_relative(path) {
            return short;
        }
        match path.strip_prefix(&self.home) {
            Some(rest) if !self.home.is_empty() => format!("~{rest}"),
            _ => path.to_owned(),
        }
    }
}
