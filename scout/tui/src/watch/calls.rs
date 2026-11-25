//! Recognising an agent's own `beagle scout` calls in a command line.

use super::*;

/// An agent's own scout call, recognised in a command line.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ScoutCall {
    Task {
        project: String,
        task: String,
        status: TaskStatus,
    },
    Log {
        message: String,
        /// The project and `--task` it was logged against, when given.
        project: String,
        task: Option<String>,
    },
    /// `beagle scout task attach`: a session joined a task.
    Attach { project: String, task: String },
}

/// The scout call a command line is about: its task change if it has one,
/// else its first call. Finds `beagle scout task <verb> …` or `beagle scout log …` anywhere in a command line,
/// including after `cd x &&`. Quoting is handled loosely: slugs never
/// contain spaces, and a log message is shown as typed minus outer quotes.
pub(crate) fn scout_call(command: &str) -> Option<ScoutCall> {
    let mut calls = scout_calls(command);
    let task = calls
        .iter()
        .position(|c| matches!(c, ScoutCall::Task { .. } | ScoutCall::Attach { .. }));
    match task {
        Some(i) => Some(calls.swap_remove(i)),
        None => calls.into_iter().next(),
    }
}

/// Every scout call in a command line, in order.
pub(crate) fn scout_calls(command: &str) -> Vec<ScoutCall> {
    // Agents often write `P=my-project; scout task start $P 3`: remember
    // simple `NAME=value` assignments and expand `$NAME` / `${NAME}` in
    // later segments.
    let mut vars: Vec<(String, String)> = Vec::new();
    let mut calls = Vec::new();
    for segment in command.split(['\n', ';', '|', '&']) {
        let segment = segment.trim();
        let assignment = segment.strip_prefix("export ").unwrap_or(segment);
        if let Some((name, value)) = assignment.split_once('=') {
            let simple = !name.is_empty()
                && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                && !value.contains(char::is_whitespace);
            if simple {
                vars.push((name.to_owned(), value.trim_matches(['"', '\'']).to_owned()));
                continue;
            }
        }
        let expanded = expand(segment, &vars);
        if let Some(call) = scout_call_in(&expanded) {
            calls.push(call);
        }
    }
    calls
}

/// Replaces `$NAME` and `${NAME}` with known values (later ones win).
pub(super) fn expand(segment: &str, vars: &[(String, String)]) -> String {
    let mut out = segment.to_owned();
    for (name, value) in vars.iter().rev() {
        out = out
            .replace(&format!("${{{name}}}"), value)
            .replace(&format!("${name}"), value);
    }
    out
}

pub(super) fn scout_call_in(segment: &str) -> Option<ScoutCall> {
    let words: Vec<&str> = segment.split_whitespace().collect();
    let at = words
        .iter()
        .position(|w| *w == "scout" || w.ends_with("/scout"))?;
    let mut args = Vec::new();
    let mut it = words[at + 1..].iter();
    while let Some(word) = it.next() {
        if *word == "--root" {
            it.next();
        } else if !word.starts_with("--root=") {
            args.push(*word);
        }
    }
    let unquote = |w: &str| w.trim_matches(['"', '\'']).to_owned();
    match args.as_slice() {
        ["task", "attach", project, task, ..] => Some(ScoutCall::Attach {
            project: unquote(project),
            task: unquote(task),
        }),
        ["task", verb, project, task, rest @ ..] => {
            let status = match *verb {
                "start" => TaskStatus::Working,
                "done" => TaskStatus::Done,
                "wait" => TaskStatus::Waiting,
                "block" => TaskStatus::Blocked,
                "skip" => TaskStatus::Skipped,
                "status" => TaskStatus::parse(&unquote(rest.first()?))?,
                _ => return None,
            };
            Some(ScoutCall::Task {
                project: unquote(project),
                task: unquote(task),
                status,
            })
        }
        ["log", project, rest @ ..] => {
            let mut words = Vec::new();
            let mut task = None;
            let mut it = rest.iter();
            while let Some(word) = it.next() {
                if *word == "--task" {
                    task = it.next().map(|t| unquote(t));
                } else if *word != "--" {
                    words.push(*word);
                }
            }
            let message = unquote(&words.join(" "));
            (!message.is_empty()).then_some(ScoutCall::Log {
                message,
                project: unquote(project),
                task,
            })
        }
        _ => None,
    }
}
