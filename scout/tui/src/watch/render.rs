//! Rendering recorded events like terminal output: commands, their output
//! (cut to a head and tail), and banners for scout's own calls.

use super::{
    duration, one_line, scout_call, Event, HashMap, Options, ScoutCall, Store, TaskStatus,
    HEAD_LINES, MAX_LINES,
};

/// ANSI styling that collapses to plain text when color is off.
pub(super) struct Paint(bool);

impl Paint {
    pub(super) fn wrap(&self, code: &str, text: &str) -> String {
        if self.0 {
            format!("\x1b[{code}m{text}\x1b[0m")
        } else {
            text.to_owned()
        }
    }
    pub(super) fn dim(&self, t: &str) -> String {
        self.wrap("2", t)
    }
    pub(super) fn bold(&self, t: &str) -> String {
        self.wrap("1", t)
    }
    pub(super) fn prompt(&self, t: &str) -> String {
        self.wrap("1;32", t)
    }
    pub(super) fn green(&self, t: &str) -> String {
        self.wrap("32", t)
    }
    pub(super) fn red(&self, t: &str) -> String {
        self.wrap("31", t)
    }
    pub(super) fn yellow(&self, t: &str) -> String {
        self.wrap("33", t)
    }
    pub(super) fn cyan(&self, t: &str) -> String {
        self.wrap("36", t)
    }
    pub(super) fn magenta(&self, t: &str) -> String {
        self.wrap("35", t)
    }
    pub(super) fn orange(&self, t: &str) -> String {
        self.wrap("1;38;5;208", t)
    }
}

/// What the renderer remembers about a command between its start and end.
pub(super) struct Pending {
    command: String,
    /// A scout banner: its output is noise, so only a failure is shown.
    quiet: bool,
}

/// Turns events into terminal text. Pure apart from looking up task titles
/// in the store, so it is unit-testable.
pub(super) struct Renderer<'a> {
    store: &'a Store,
    pub(super) paint: Paint,
    commands_only: bool,
    full: bool,
    pending: HashMap<String, Pending>,
    /// The command printed most recently, so its output can follow it
    /// directly; output for any other command gets a `↳` header.
    last: Option<String>,
}

impl<'a> Renderer<'a> {
    pub(super) fn new(store: &'a Store, options: &Options) -> Self {
        Self {
            store,
            paint: Paint(options.color),
            commands_only: options.commands_only,
            full: options.full,
            pending: HashMap::new(),
            last: None,
        }
    }

    pub(super) fn reset(&mut self) {
        self.pending.clear();
        self.last = None;
    }

    pub(super) fn render(&mut self, event: &Event) -> String {
        match event {
            Event::Start {
                id,
                command,
                description,
                agent,
                background,
                ..
            } => self.start(
                id,
                command,
                description.as_deref(),
                agent.as_deref(),
                *background,
            ),
            Event::End {
                id,
                stdout,
                stderr,
                interrupted,
                duration_ms,
                ..
            } => self.end(id, stdout, stderr, *interrupted, *duration_ms),
            Event::Fail {
                id,
                error,
                interrupt,
                duration_ms,
                ..
            } => self.fail(id, error, *interrupt, *duration_ms),
        }
    }

    pub(super) fn start(
        &mut self,
        id: &str,
        command: &str,
        description: Option<&str>,
        agent: Option<&str>,
        background: bool,
    ) -> String {
        let p = &self.paint;
        let mut text = String::from("\n");
        if let Some(banner) = self.banner(command) {
            text.push_str(&banner);
            text.push('\n');
            self.pending.insert(
                id.to_owned(),
                Pending {
                    command: command.to_owned(),
                    quiet: true,
                },
            );
            self.last = Some(id.to_owned());
            return text;
        }
        let mut note = Vec::new();
        if let Some(agent) = agent {
            note.push(p.magenta(&format!("[{agent}]")));
        }
        if let Some(description) = description {
            note.push(p.dim(&format!("# {description}")));
        }
        if background {
            note.push(p.yellow("(background)"));
        }
        if !note.is_empty() {
            text.push_str(&note.join(" "));
            text.push('\n');
        }
        for (i, line) in command.lines().enumerate() {
            let lead = if i == 0 { p.prompt("$ ") } else { p.dim("> ") };
            text.push_str(&lead);
            text.push_str(&p.bold(line));
            text.push('\n');
        }
        self.pending.insert(
            id.to_owned(),
            Pending {
                command: command.to_owned(),
                quiet: false,
            },
        );
        self.last = Some(id.to_owned());
        text
    }

    pub(super) fn end(
        &mut self,
        id: &str,
        stdout: &str,
        stderr: &str,
        interrupted: bool,
        duration_ms: Option<u64>,
    ) -> String {
        let pending = self.pending.remove(id);
        if pending.as_ref().is_some_and(|p| p.quiet) && !interrupted {
            return String::new();
        }
        let mut text = self.reattach(id, pending.as_ref());
        if !self.commands_only {
            text.push_str(&self.output(stdout, false));
            text.push_str(&self.output(stderr, true));
        }
        let status = if interrupted {
            self.paint.yellow("⏹ interrupted")
        } else {
            self.paint.green(&format!("✔{}", duration(duration_ms)))
        };
        text.push_str(&status);
        text.push('\n');
        text
    }

    pub(super) fn fail(
        &mut self,
        id: &str,
        error: &str,
        interrupt: bool,
        duration_ms: Option<u64>,
    ) -> String {
        let pending = self.pending.remove(id);
        let mut text = self.reattach(id, pending.as_ref());
        let (code, body) = match error.split_once('\n') {
            Some((first, rest)) if first.starts_with("Exit code ") => (Some(first), rest),
            None if error.starts_with("Exit code ") => (Some(error), ""),
            _ => (None, error),
        };
        if !self.commands_only {
            text.push_str(&self.output(body, false));
        }
        let status = if interrupt {
            self.paint.yellow("⏹ interrupted")
        } else {
            let code = code.map_or_else(
                || "failed".to_owned(),
                |c| c.replacen("Exit code ", "exit ", 1),
            );
            self.paint.red(&format!(
                "✖ {code}{}",
                duration(duration_ms).replacen(' ', " · ", 1)
            ))
        };
        text.push_str(&status);
        text.push('\n');
        text
    }

    /// When output arrives for a command other than the one printed last
    /// (commands ran concurrently), name the command it belongs to.
    pub(super) fn reattach(&mut self, id: &str, pending: Option<&Pending>) -> String {
        let follows = self.last.as_deref() == Some(id);
        self.last = None;
        match pending {
            Some(p) if !follows => format!(
                "{}\n",
                self.paint.dim(&format!("↳ {}", one_line(&p.command, 80)))
            ),
            _ => String::new(),
        }
    }

    pub(super) fn output(&self, text: &str, is_stderr: bool) -> String {
        let text = text.trim_end_matches('\n');
        if text.is_empty() {
            return String::new();
        }
        let lines: Vec<&str> = text.lines().collect();
        let shown: Vec<String> = if self.full || lines.len() <= MAX_LINES {
            lines.iter().map(|l| (*l).to_owned()).collect()
        } else {
            let tail = MAX_LINES - HEAD_LINES;
            let hidden = lines.len() - MAX_LINES;
            let mut shown: Vec<String> = lines[..HEAD_LINES]
                .iter()
                .map(|l| (*l).to_owned())
                .collect();
            shown.push(
                self.paint
                    .dim(&format!("⋮ {hidden} more lines (--full shows everything)")),
            );
            shown.extend(lines[lines.len() - tail..].iter().map(|l| (*l).to_owned()));
            shown
        };
        let mut out = String::new();
        for line in shown {
            if is_stderr {
                out.push_str(&self.paint.red(&line));
            } else {
                out.push_str(&line);
            }
            out.push('\n');
        }
        out
    }

    /// A banner for the agent's own scout calls, or `None` for anything else.
    pub(super) fn banner(&self, command: &str) -> Option<String> {
        let p = &self.paint;
        match scout_call(command)? {
            ScoutCall::Task {
                project,
                task,
                status,
            } => {
                let (id, title) = self
                    .store
                    .project(&project)
                    .ok()
                    .and_then(|proj| {
                        proj.resolve_task(&task)
                            .map(|t| (t.id.clone(), t.meta.title.clone()))
                    })
                    .unwrap_or((task, String::new()));
                let title = if title.is_empty() {
                    String::new()
                } else {
                    format!(" · {title}")
                };
                Some(match status {
                    TaskStatus::Working => p.orange(&format!("▶ task {id}{title}")),
                    TaskStatus::Paused => p.yellow(&format!("‖ task {id} paused{title}")),
                    TaskStatus::Done => p.green(&format!("✔ task {id} done{title}")),
                    TaskStatus::Blocked => p.red(&format!("■ task {id} blocked{title}")),
                    TaskStatus::Skipped => p.dim(&format!("– task {id} skipped{title}")),
                    TaskStatus::Todo => p.dim(&format!("○ task {id} back to todo{title}")),
                })
            }
            ScoutCall::Log { message, .. } => Some(p.cyan(&format!("✎ {message}"))),
            ScoutCall::Attach { task, .. } => {
                Some(p.dim(&format!("⚓ session attached to task {task}")))
            }
        }
    }
}
