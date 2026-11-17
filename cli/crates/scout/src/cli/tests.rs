//! Parser tests: every subcommand shape, and the usage errors.

// Tests fail loudly on unexpected shapes; unwrap/panic is the clearest way.
#![allow(clippy::unwrap_used, clippy::panic)]

use super::*;

#[test]
fn help_and_version_flags() {
    assert_eq!(command("--help"), Command::Help);
    assert_eq!(command("help"), Command::Help);
    assert!(USAGE.contains("beagle scout list"));
}

fn parse(line: &str) -> Result<Invocation, String> {
    parse_args(line.split_whitespace().map(str::to_owned))
}

fn command(line: &str) -> Command {
    parse(line).unwrap().command
}

#[test]
fn bare_is_tui_and_root_lifts_out() {
    assert_eq!(command(""), Command::Tui);
    let inv = parse("--root /tmp/x list").unwrap();
    assert_eq!(inv.root, Some(PathBuf::from("/tmp/x")));
    assert_eq!(inv.command, Command::List { json: false });
    assert_eq!(
        parse("list --root=/a").unwrap().root,
        Some(PathBuf::from("/a"))
    );
    assert!(parse("--root").is_err());
}

#[test]
fn new_collects_links() {
    assert_eq!(
        command("new p --title T --link ABC-1 --link=backlog:TASK-2"),
        Command::New {
            project: "p".into(),
            title: "T".into(),
            links: vec!["ABC-1".into(), "backlog:TASK-2".into()],
        }
    );
    assert!(parse("new p").is_err());
    assert!(parse("new --title T").is_err());
    assert!(parse("new p --title T --owner me").is_err());
}

#[test]
fn task_shortcuts_map_to_statuses() {
    for (word, status) in [
        ("start", TaskStatus::Working),
        ("done", TaskStatus::Done),
        ("block", TaskStatus::Blocked),
        ("skip", TaskStatus::Skipped),
    ] {
        assert_eq!(
            command(&format!("task {word} p 3")),
            Command::TaskStatus {
                project: "p".into(),
                task: "3".into(),
                status,
                reason: None
            }
        );
    }
    assert_eq!(
        command("task status p 03-x blocked"),
        Command::TaskStatus {
            project: "p".into(),
            task: "03-x".into(),
            status: TaskStatus::Blocked,
            reason: None
        }
    );
    assert!(parse("task status p 3 in-progress").is_err());
    assert!(parse("task start p").is_err());
    assert_eq!(
        command("task block p 3 --reason needs-approval"),
        Command::TaskStatus {
            project: "p".into(),
            task: "3".into(),
            status: TaskStatus::Blocked,
            reason: Some("needs-approval".into()),
        }
    );
}

#[test]
fn log_joins_words_and_takes_a_task() {
    assert_eq!(
        command("log p --task 2 checking the pool size"),
        Command::Log {
            project: "p".into(),
            task: Some("2".into()),
            message: "checking the pool size".into(),
        }
    );
    assert_eq!(
        command("log p -- --weird message"),
        Command::Log {
            project: "p".into(),
            task: None,
            message: "--weird message".into()
        }
    );
    assert!(parse("log p").is_err());
}

#[test]
fn misc() {
    assert_eq!(command("ls --json"), Command::List { json: true });
    assert_eq!(
        command("status p done"),
        Command::ProjectStatus {
            project: "p".into(),
            status: ProjectStatus::Done
        }
    );
    assert_eq!(
        command("path p 2"),
        Command::Path {
            project: "p".into(),
            task: Some("2".into())
        }
    );
    assert_eq!(
        command("link p --task 1 https://x"),
        Command::Link {
            project: "p".into(),
            task: Some("1".into()),
            link: "https://x".into()
        }
    );
    assert_eq!(command("skill install"), Command::SkillInstall);
    assert_eq!(command("hook"), Command::Hook);
    let Command::Watch(w) = command("watch 4f85 --full") else {
        panic!("expected watch")
    };
    assert_eq!(
        (w.session.as_deref(), w.full, w.auto),
        (Some("4f85"), true, false)
    );
    let Command::Watch(w) = command("watch -a --once --color") else {
        panic!("expected watch")
    };
    assert!(w.auto && w.once && w.color);
    assert!(parse("watch --auto 4f85").is_err());
    assert!(parse("watch a b").is_err());
    assert_eq!(command("watch --list"), Command::WatchList);
    assert_eq!(command("hook install"), Command::HookInstall);
    assert_eq!(
        command("task attach p 2 --session abc"),
        Command::TaskAttach {
            project: "p".into(),
            task: "2".into(),
            session: Some("abc".into())
        }
    );
    assert_eq!(
        command("session show p 2 -f"),
        Command::SessionShow {
            project: "p".into(),
            task: "2".into(),
            follow: true
        }
    );
    assert!(parse("hook frob").is_err());
    assert_eq!(command("-V"), Command::Version);
    assert!(parse("frobnicate").is_err());
    assert!(parse("status p finished").is_err());
}
