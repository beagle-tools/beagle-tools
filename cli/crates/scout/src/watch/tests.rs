//! watch tests: rendering, banners and scout-call detection.

// Tests fail loudly on setup errors; unwrap/expect/panic is the clearest way.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;
use crate::model::Slug;
use std::io::Write as _;
use time::macros::datetime;

fn options() -> Options {
    Options {
        session: None,
        history: 20,
        commands_only: false,
        full: false,
        color: false,
        once: false,
        tab: None,
        cwd: PathBuf::from("/"),
    }
}

fn start(id: &str, command: &str) -> Event {
    Event::Start {
        ts: datetime!(2026-11-12 15:00 UTC),
        id: id.into(),
        command: command.into(),
        description: Some("Run the tests".into()),
        cwd: None,
        agent: None,
        background: false,
        tab: None,
        pane: None,
        transcript: None,
    }
}

fn end(id: &str, stdout: &str) -> Event {
    Event::End {
        ts: datetime!(2026-11-12 15:00 UTC),
        id: id.into(),
        stdout: stdout.into(),
        stderr: String::new(),
        interrupted: false,
        duration_ms: Some(2300),
    }
}

#[test]
fn a_command_reads_like_a_terminal() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path());
    let mut r = Renderer::new(&store, &options());
    let text = r.render(&start("1", "cargo test\n  --all")) + &r.render(&end("1", "ok\n"));
    assert_eq!(
        text,
        "\n# Run the tests\n$ cargo test\n>   --all\nok\n✔ 2.3s\n"
    );
}

#[test]
fn failures_show_the_exit_code() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path());
    let mut r = Renderer::new(&store, &options());
    r.render(&start("1", "false"));
    let fail = Event::Fail {
        ts: datetime!(2026-11-12 15:00 UTC),
        id: "1".into(),
        error: "Exit code 1\nboom".into(),
        interrupt: false,
        duration_ms: Some(40),
    };
    assert_eq!(r.render(&fail), "boom\n✖ exit 1 · 40ms\n");
}

#[test]
fn interleaved_output_names_its_command() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path());
    let mut r = Renderer::new(&store, &options());
    r.render(&start("1", "sleep 5"));
    r.render(&start("2", "ls"));
    let text = r.render(&end("1", ""));
    assert!(text.starts_with("↳ sleep 5\n"), "{text}");
}

#[test]
fn long_output_keeps_head_and_tail() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path());
    let r = Renderer::new(&store, &options());
    let long = (0..100)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let shown = r.output(&long, false);
    assert_eq!(shown.lines().count(), MAX_LINES + 1);
    assert!(shown.contains("⋮ 60 more lines"));
    assert!(shown.starts_with("line 0\n"));
    assert!(shown.ends_with("line 99\n"));
}

#[test]
fn commands_only_hides_output() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path());
    let mut r = Renderer::new(
        &store,
        &Options {
            commands_only: true,
            ..options()
        },
    );
    r.render(&start("1", "ls"));
    assert_eq!(r.render(&end("1", "a\nb\n")), "✔ 2.3s\n");
}

#[test]
fn scout_calls_become_banners() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path());
    store
        .create_project(&Slug::parse("p").unwrap(), "P", vec![])
        .unwrap();
    store
        .add_task("p", &Slug::parse("map").unwrap(), "Map the path", vec![])
        .unwrap();
    let mut r = Renderer::new(&store, &options());
    assert_eq!(
        r.render(&start("1", "scout task start p 1")),
        "\n▶ task 01-map · Map the path\n"
    );
    assert_eq!(r.render(&end("1", "01-map: todo → working")), "");
    assert_eq!(
        r.render(&start(
            "2",
            "cd /x && ./target/debug/scout log p --task 1 \"found it\""
        )),
        "\n✎ found it\n"
    );
    assert_eq!(
        r.render(&start("3", "scout task done p 9")),
        "\n✔ task 9 done\n"
    );
}

#[test]
fn parses_scout_calls() {
    assert_eq!(
        scout_call("scout --root /r task status p 03-x blocked"),
        Some(ScoutCall::Task {
            project: "p".into(),
            task: "03-x".into(),
            status: TaskStatus::Blocked
        })
    );
    assert_eq!(scout_call("scout task add p x --title T"), None);
    assert_eq!(
        scout_call("P=my-proj; scout log $P --task 1 \"hi\"; scout task start ${P} 3 >/dev/null"),
        Some(ScoutCall::Task {
            project: "my-proj".into(),
            task: "3".into(),
            status: TaskStatus::Working
        })
    );
    assert_eq!(
        scout_call("export P=\"q\" && scout task attach $P 2"),
        Some(ScoutCall::Attach {
            project: "q".into(),
            task: "2".into()
        })
    );
    assert_eq!(scout_call("echo scout"), None);
    assert_eq!(scout_call("scouts task start p 1"), None);
}

#[test]
fn parses_beagle_scout_calls() {
    assert_eq!(
        scout_call("beagle scout task start p 2"),
        Some(ScoutCall::Task {
            project: "p".into(),
            task: "2".into(),
            status: TaskStatus::Working
        })
    );
    assert_eq!(
        scout_call("/usr/local/bin/beagle scout --root /r log p --task 1 hi"),
        Some(ScoutCall::Log {
            message: "hi".into(),
            project: "p".into(),
            task: Some("1".into()),
        })
    );
}

#[test]
fn follows_appends_and_partial_lines() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("s.jsonl");
    let line = serde_json::to_string(&start("1", "ls")).unwrap();
    std::fs::write(&path, format!("{line}\n{}", &line[..10])).unwrap();
    let mut a = Attached {
        id: "s".into(),
        path: path.clone(),
        offset: 0,
        partial: Vec::new(),
    };
    assert_eq!(a.read_new().unwrap().len(), 1);
    let mut f = std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap();
    writeln!(f, "{}", &line[10..]).unwrap();
    assert_eq!(a.read_new().unwrap(), vec![start("1", "ls")]);
    assert_eq!(a.read_new().unwrap().len(), 0);
}

#[test]
fn formats() {
    assert_eq!(duration(Some(340)), " 340ms");
    assert_eq!(duration(Some(65_000)), " 1m05s");
    assert_eq!(duration(None), "");
    assert_eq!(one_line("a\nb", 10), "a…");
    assert_eq!(human_age(Duration::from_secs(7200)), "2h");
}
