//! What the content pane shows: a task's file, its handoff, or its
//! attached sessions, loaded from disk.

use super::*;

/// The Session tab: the task's attached sessions, as the pipeline rendered
/// them into `<project>/sessions/<id>.log`. Displaying only reads that file;
/// it never parses a transcript.
pub(super) fn session_text(task_file: &std::path::Path, source: &str) -> Text<'static> {
    let dim = Style::default()
        .fg(Color::DarkGray)
        .add_modifier(Modifier::ITALIC);
    let sessions = frontmatter::parse::<crate::model::TaskMeta>(task_file, source)
        .map(|(meta, _)| meta.sessions)
        .unwrap_or_default();
    // <root>/<project>/tasks/<task>.md
    let project_dir = task_file.parent().and_then(std::path::Path::parent);
    let (Some(project_dir), false) = (project_dir, sessions.is_empty()) else {
        return Text::from(vec![
            Line::styled("No session attached to this task.", dim),
            Line::default(),
            Line::styled(
                "An agent attaches its Claude session with `beagle scout task attach <project> <task>` \
                 (`beagle scout task start` does it automatically). What it made, its commands, \
                 thinking and replies then stream in here.",
                dim,
            ),
        ]);
    };
    let task_id = task_file
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut lines = Vec::new();
    for id in sessions {
        let view = transcript::view_file(&transcript::cache_file(project_dir, &id), &task_id);
        if !lines.is_empty() {
            lines.push(Line::default());
        }
        match std::fs::read_to_string(&view) {
            Ok(text) => {
                lines.extend(transcript_view::from_ansi(&recent(&text, &id, &task_id)).lines);
            }
            Err(_) => lines.push(Line::styled(
                format!(
                    "Session {} is being prepared… (it appears after the agent's next step)",
                    id.get(..8).unwrap_or(&id)
                ),
                dim,
            )),
        }
    }
    Text::from(lines)
}

/// The most timeline lines the Session tab shows; older ones are summarized
/// in one line (`beagle scout session show` prints everything).
pub(super) const SESSION_TAIL: usize = 1000;

/// Keeps a view's summary (everything up to `── timeline ──`) and the last
/// [`SESSION_TAIL`] timeline lines, so a very long task never slows the tab.
pub(super) fn recent(view: &str, session: &str, task_id: &str) -> String {
    let lines: Vec<&str> = view.lines().collect();
    let body = lines
        .iter()
        .position(|l| l.contains("── timeline ──"))
        .map_or(0, |i| i + 1);
    let timeline = lines.len() - body;
    if timeline <= SESSION_TAIL {
        return view.to_owned();
    }
    let hidden = timeline - SESSION_TAIL;
    let number = task_id.split('-').next().unwrap_or(task_id);
    let mut out = lines[..body].join("\n");
    let short = session.get(..8).unwrap_or(session);
    let _ = writeln!(
        out,
        "\n\x1b[90m⋮ {hidden} earlier lines hidden — `beagle scout session show <project> {number}` \
         prints all of session {short}\x1b[0m"
    );
    out.push_str(&lines[lines.len() - SESSION_TAIL..].join("\n"));
    out
}

/// The handoff sections' content without their headings, for the
/// "is anything written yet?" check.
pub(super) fn handoff_text(handoff: &str) -> String {
    handoff
        .lines()
        .filter(|l| !l.trim_start().starts_with("## "))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The task to land on when entering a project: the first one being worked,
/// else the first still open, else the first.
pub(super) fn default_task(project: &Project) -> usize {
    let tasks = &project.tasks;
    tasks
        .iter()
        .position(|t| t.meta.status == TaskStatus::Working)
        .or_else(|| tasks.iter().position(|t| !t.meta.status.is_closed()))
        .unwrap_or(0)
}

/// Reads and renders (part of) a file for the content pane; failures render
/// as a message instead of an error.
pub(super) fn load_pane(path: &std::path::Path, view: View) -> Text<'static> {
    let dim = Style::default()
        .fg(Color::DarkGray)
        .add_modifier(Modifier::ITALIC);
    match store::read_capped(path) {
        Ok(source) => {
            let body = frontmatter::body(&source);
            let (main, handoff) = markdown::split_sections(body, &store::HANDOFF_SECTIONS);
            match view {
                View::Whole => markdown::to_text(body.trim_start_matches('\n')),
                View::TaskMain => markdown::to_text(main.trim_start_matches('\n')),
                View::Handoff if store::is_placeholder(&handoff_text(&handoff)) => {
                    Text::from(vec![
                        Line::styled("No outcome or next steps yet.", dim),
                        Line::default(),
                        Line::styled(
                            "The agent writes ## Outcome and ## Next steps when it closes or \
                             pauses this task, so the next task (or session) knows what to do.",
                            dim,
                        ),
                    ])
                }
                View::Handoff => markdown::to_text(handoff.trim_start_matches('\n')),
                View::Session => session_text(path, &source),
            }
        }
        Err(Error::Io { source, .. }) if source.kind() == ErrorKind::NotFound => {
            let hint = if path.file_name().is_some_and(|n| n == store::LOG_FILE) {
                "No activity yet. Agents append here with `beagle scout log`."
            } else {
                "This file does not exist yet."
            };
            Text::from(Line::styled(hint, dim))
        }
        Err(e) => Text::from(Line::styled(
            e.to_string(),
            Style::default().fg(Color::LightRed),
        )),
    }
}
