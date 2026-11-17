//! Argument parsing. Pure: no filesystem, no environment — [`crate::app`]
//! does the I/O.

mod args;
mod sub;
mod usage;

use std::path::PathBuf;

use crate::model::{ProjectStatus, TaskStatus};

use args::Args;
pub(crate) use usage::USAGE;

/// A parsed command line.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Invocation {
    pub root: Option<PathBuf>,
    pub command: Command,
}

/// One `beagle scout` subcommand and its arguments.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Command {
    Tui,
    List {
        json: bool,
    },
    Show {
        project: String,
    },
    Path {
        project: String,
        task: Option<String>,
    },
    New {
        project: String,
        title: String,
        links: Vec<String>,
    },
    ProjectStatus {
        project: String,
        status: ProjectStatus,
    },
    Link {
        project: String,
        task: Option<String>,
        link: String,
    },
    TaskAdd {
        project: String,
        slug: String,
        title: String,
        links: Vec<String>,
    },
    TaskStatus {
        project: String,
        task: String,
        status: TaskStatus,
        /// `--reason`: logged, and shown in the notification (for `block`).
        reason: Option<String>,
    },
    TaskRm {
        project: String,
        task: String,
    },
    TaskAttach {
        project: String,
        task: String,
        session: Option<String>,
    },
    SessionShow {
        project: String,
        task: String,
        follow: bool,
    },
    Log {
        project: String,
        task: Option<String>,
        message: String,
    },
    Watch(WatchArgs),
    WatchList,
    Hook,
    HookInstall,
    HookUninstall,
    HookStatus,
    NotifyTest,
    NotifySetup,
    SkillInstall,
    SkillStatus,
    Version,
    Help,
}

/// `beagle scout watch` flags.
#[derive(Debug, PartialEq, Eq)]
// Independent command-line flags, not a state machine.
#[allow(clippy::struct_excessive_bools)]
pub(crate) struct WatchArgs {
    pub session: Option<String>,
    pub history: usize,
    pub commands_only: bool,
    pub full: bool,
    /// Skip the picker and follow this tab's newest session.
    pub auto: bool,
    /// Print the replay and exit (used by the picker's preview).
    pub once: bool,
    /// Force ANSI colors even when stdout is not a terminal.
    pub color: bool,
}

/// Parses everything after `scout`, lifting out `--root` wherever it is.
pub(crate) fn parse_args(raw: impl IntoIterator<Item = String>) -> Result<Invocation, String> {
    let mut root = None;
    let mut rest = Vec::new();
    let mut it = raw.into_iter();
    while let Some(arg) = it.next() {
        if arg == "--root" {
            root = Some(PathBuf::from(it.next().ok_or("--root needs a directory")?));
        } else if let Some(dir) = arg.strip_prefix("--root=") {
            root = Some(PathBuf::from(dir));
        } else {
            rest.push(arg);
        }
    }
    Ok(Invocation {
        root,
        command: parse_command(&rest)?,
    })
}

fn parse_command(args: &[String]) -> Result<Command, String> {
    let Some((first, rest)) = args.split_first() else {
        return Ok(Command::Tui);
    };
    match first.as_str() {
        "list" | "ls" => match rest {
            [] => Ok(Command::List { json: false }),
            [flag] if flag == "--json" => Ok(Command::List { json: true }),
            _ => Err("usage: beagle scout list [--json]".to_owned()),
        },
        "show" => {
            let [project] = Args::parse(rest, &[])?.exact(["project"])?;
            Ok(Command::Show { project })
        }
        "path" => {
            let args = Args::parse(rest, &[])?;
            match args.positionals.as_slice() {
                [project] => Ok(Command::Path {
                    project: project.clone(),
                    task: None,
                }),
                [project, task] => Ok(Command::Path {
                    project: project.clone(),
                    task: Some(task.clone()),
                }),
                _ => Err("usage: beagle scout path <project> [<task>]".to_owned()),
            }
        }
        "new" => {
            let args = Args::parse(rest, &["title", "link"])?;
            let [project] = args.exact(["project"])?;
            let title = args.one("title").ok_or("beagle scout new needs --title")?;
            Ok(Command::New {
                project,
                title,
                links: args.all("link"),
            })
        }
        "status" => {
            let [project, status] = Args::parse(rest, &[])?.exact(["project", "status"])?;
            let status = ProjectStatus::parse(&status).ok_or_else(|| {
                format!("unknown project status {status:?}: planning|active|paused|done|dropped")
            })?;
            Ok(Command::ProjectStatus { project, status })
        }
        "link" => {
            let args = Args::parse(rest, &["task"])?;
            let [project, link] = args.exact(["project", "ref"])?;
            Ok(Command::Link {
                project,
                task: args.one("task"),
                link,
            })
        }
        "log" => parse_log(rest),
        "task" => sub::parse_task(rest),
        "watch" => sub::parse_watch(rest),
        "session" => sub::parse_session(rest),
        "hook" => match rest {
            [] => Ok(Command::Hook),
            [a] if a == "install" => Ok(Command::HookInstall),
            [a] if a == "uninstall" => Ok(Command::HookUninstall),
            [a] if a == "status" => Ok(Command::HookStatus),
            _ => Err("usage: beagle scout hook [install | uninstall | status]".to_owned()),
        },
        "notify" => match rest {
            [a] if a == "test" => Ok(Command::NotifyTest),
            [a] if a == "setup" => Ok(Command::NotifySetup),
            _ => Err("usage: beagle scout notify setup | test".to_owned()),
        },
        "skill" => match rest {
            [a] if a == "install" => Ok(Command::SkillInstall),
            [a] if a == "status" => Ok(Command::SkillStatus),
            _ => Err("usage: beagle scout skill install | status".to_owned()),
        },
        "version" | "--version" | "-V" => Ok(Command::Version),
        "help" | "--help" | "-h" => Ok(Command::Help),
        other => Err(format!("unknown command {other:?}")),
    }
}

fn parse_log(rest: &[String]) -> Result<Command, String> {
    let args = Args::parse(rest, &["task"])?;
    let Some((project, words)) = args.positionals.split_first() else {
        return Err("usage: beagle scout log <project> [--task <task>] <message...>".to_owned());
    };
    let message = words.join(" ");
    if message.trim().is_empty() {
        return Err("beagle scout log needs a message".to_owned());
    }
    Ok(Command::Log {
        project: project.clone(),
        task: args.one("task"),
        message,
    })
}

#[cfg(test)]
mod tests;
