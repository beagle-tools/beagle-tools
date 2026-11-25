//! Parsers for the subcommands with their own sub-subcommands: `task`,
//! `session` and `watch`.

use crate::model::TaskStatus;

use super::args::Args;
use super::{Command, WatchArgs};

pub(super) fn parse_watch(args: &[String]) -> Result<Command, String> {
    let mut watch = WatchArgs {
        session: None,
        history: 20,
        commands_only: false,
        full: false,
        auto: false,
        once: false,
        color: false,
    };
    let mut list = false;
    let mut rest = Vec::new();
    for arg in args {
        match arg.as_str() {
            "--list" => list = true,
            "--commands-only" => watch.commands_only = true,
            "--full" => watch.full = true,
            "--auto" | "-a" => watch.auto = true,
            "--once" => watch.once = true,
            "--color" => watch.color = true,
            _ => rest.push(arg.clone()),
        }
    }
    let parsed = Args::parse(&rest, &["session", "history"])?;
    // `beagle scout watch <id>` is shorthand for `--session <id>`.
    let positional = match parsed.positionals.as_slice() {
        [] => None,
        [id] => Some(id.clone()),
        _ => {
            return Err(
                "usage: beagle scout watch [<session>] [--auto] [--history <n>] \
                 [--commands-only] [--full] [--once] | --list"
                    .to_owned(),
            )
        }
    };
    if list {
        return Ok(Command::WatchList);
    }
    watch.session = parsed.one("session").or(positional);
    if watch.auto && watch.session.is_some() {
        return Err("use either --auto or a session id, not both".to_owned());
    }
    if let Some(n) = parsed.one("history") {
        watch.history = n
            .parse()
            .map_err(|_| format!("--history needs a number, got {n:?}"))?;
    }
    Ok(Command::Watch(watch))
}

pub(super) fn parse_session(rest: &[String]) -> Result<Command, String> {
    let Some((sub, rest)) = rest.split_first() else {
        return Err("usage: beagle scout session show <project> <task> [--follow]".to_owned());
    };
    if sub != "show" {
        return Err(format!("unknown session command {sub:?}"));
    }
    let follow = rest.iter().any(|a| a == "--follow" || a == "-f");
    let rest: Vec<String> = rest
        .iter()
        .filter(|a| *a != "--follow" && *a != "-f")
        .cloned()
        .collect();
    let [project, task] = Args::parse(&rest, &[])?.exact(["project", "task"])?;
    Ok(Command::SessionShow {
        project,
        task,
        follow,
    })
}

pub(super) fn parse_task(args: &[String]) -> Result<Command, String> {
    let Some((sub, rest)) = args.split_first() else {
        return Err(
            "usage: beagle scout task add|start|wait|done|block|skip|status|rm ...".to_owned(),
        );
    };
    let shortcut = match sub.as_str() {
        "start" => Some(TaskStatus::Working),
        "done" => Some(TaskStatus::Done),
        "wait" => Some(TaskStatus::Waiting),
        "block" => Some(TaskStatus::Blocked),
        "skip" => Some(TaskStatus::Skipped),
        "pause" => Some(TaskStatus::Paused),
        _ => None,
    };
    if let Some(status) = shortcut {
        let args = Args::parse(rest, &["reason"])?;
        let [project, task] = args.exact(["project", "task"])?;
        return Ok(Command::TaskStatus {
            project,
            task,
            status,
            reason: args.one("reason"),
        });
    }
    match sub.as_str() {
        "add" => {
            let args = Args::parse(rest, &["title", "link"])?;
            let [project, slug] = args.exact(["project", "slug"])?;
            let title = args
                .one("title")
                .ok_or("beagle scout task add needs --title")?;
            Ok(Command::TaskAdd {
                project,
                slug,
                title,
                links: args.all("link"),
            })
        }
        "status" => {
            let args = Args::parse(rest, &["reason"])?;
            let [project, task, status] = args.exact(["project", "task", "status"])?;
            let status = TaskStatus::parse(&status).ok_or_else(|| {
                format!("unknown task status {status:?}: todo|working|paused|blocked|done|skipped")
            })?;
            Ok(Command::TaskStatus {
                project,
                task,
                status,
                reason: args.one("reason"),
            })
        }
        "rm" => {
            let [project, task] = Args::parse(rest, &[])?.exact(["project", "task"])?;
            Ok(Command::TaskRm { project, task })
        }
        "attach" => {
            let args = Args::parse(rest, &["session"])?;
            let [project, task] = args.exact(["project", "task"])?;
            Ok(Command::TaskAttach {
                project,
                task,
                session: args.one("session"),
            })
        }
        other => Err(format!("unknown task command {other:?}")),
    }
}
