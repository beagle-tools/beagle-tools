//! Session view rendering tests.

// Tests fail loudly on setup errors; unwrap/expect/panic is the clearest way.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;
use crate::model::Slug;

fn plain(text: &Text<'_>) -> String {
    text.lines
        .iter()
        .map(|l| {
            l.spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn tool(id: &str, name: &str, summary: &str, detail: Option<&str>) -> Entry {
    Entry::Tool {
        ts: "2026-11-16T15:05:00Z".into(),
        id: id.into(),
        name: name.into(),
        summary: summary.into(),
        detail: detail.map(str::to_owned),
    }
}

fn result(id: &str, text: &str, error: bool) -> Entry {
    Entry::Result {
        ts: "t".into(),
        id: id.into(),
        text: text.into(),
        error,
    }
}

#[test]
fn summary_then_a_readable_timeline() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path());
    store
        .create_project(&Slug::parse("p").unwrap(), "P", vec![])
        .unwrap();
    store
        .add_task(
            "p",
            &Slug::parse("fix").unwrap(),
            "Fix the retry count",
            vec![],
        )
        .unwrap();
    let entries = vec![
        Entry::Prompt {
            ts: "2026-11-16T15:04:00Z".into(),
            text: "fix the flaky test".into(),
            cwd: Some("/repo".into()),
        },
        Entry::Thinking {
            ts: "t".into(),
            text: None,
            ms: Some(4000),
        },
        Entry::Text {
            ts: "t".into(),
            text: "Running the **tests**.".into(),
        },
        tool("1", "Bash", "scout task start p 1", None),
        result("1", "01-fix: todo → working", false),
        tool("2", "Read", "/repo/src/a.rs", None),
        tool("3", "Read", "/repo/src/b.rs", None),
        result("2", "…", false),
        tool(
            "4",
            "Bash",
            "cargo test uploader --all-features",
            Some("Run the uploader tests"),
        ),
        result("4", "Exit code 101\ntest retry ... FAILED", true),
        tool("5", "Edit", "/repo/src/a.rs", None),
        result("5", "ok", false),
        Entry::Change {
            ts: "2026-11-16T15:09:00Z".into(),
            id: "5".into(),
            path: "/repo/src/a.rs".into(),
            created: false,
            added: 2,
            removed: 1,
        },
    ];
    let text = plain(&render(&[("1163b504-abcd".into(), entries)], &store));
    let expected = [
        "━━ session 1163b504 · 15:04 UTC → 15:09 UTC ━━",
        "1 prompt(s) · 1 command(s) (1 failed) · 1 file(s) changed",
        "  ✎ src/a.rs  +2 −1",
        "── timeline ──",
        "",
        "› You · 15:04 UTC",
        "  fix the flaky test",
        "∴ thought for 4.0s",
        "",
        "● Running the tests.",
        "",
        "▶ task 01-fix · Fix the retry count",
        "▸ looked at a.rs, b.rs",
        "",
        "▸ Run the uploader tests",
        "  $ cargo test uploader --all-features",
        "  │ test retry ... FAILED",
        "  ✖ exit 101",
        "✎ edited  src/a.rs  +2 −1",
    ]
    .join("\n");
    assert_eq!(text, expected);
}

#[test]
fn long_commands_and_output_stay_short() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path());
    let long_cmd = format!(
        "S=/private/tmp/{}; docker run\n  set -e\n  ls",
        "x".repeat(200)
    );
    let output = (0..30)
        .map(|i| format!("l{i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let entries = vec![
        tool("1", "Bash", &long_cmd, Some("Build")),
        result("1", &output, false),
    ];
    let text = plain(&render(&[("s".into(), entries)], &store));
    let command = text.lines().find(|l| l.starts_with("  $ ")).unwrap();
    assert!(command.ends_with("…  (+2 lines)"), "{command}");
    assert!(command.chars().count() < COMMAND_WIDTH + 20);
    assert!(
        text.contains("  │ l2\n  │ ⋮ 24 more lines\n  │ l27"),
        "{text}"
    );
}

#[test]
fn ansi_round_trips() {
    let original = Text::from(vec![
        Line::from(vec![
            Span::styled(
                "$ ",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("ls"),
            Span::styled(" ▶", Style::default().fg(Color::Indexed(208))),
        ]),
        Line::default(),
        Line::styled(
            "dim",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        ),
    ]);
    let ansi = to_ansi(&original);
    assert_eq!(
        ansi.lines().next().unwrap(),
        "\x1b[1;32m$ \x1b[0mls\x1b[38;5;208m ▶\x1b[0m"
    );
    let back = from_ansi(&ansi);
    assert_eq!(back.lines.len(), 3);
    assert_eq!(plain(&back), plain(&original));
    assert_eq!(back.lines[0].spans[0].style.fg, Some(Color::Green));
    assert!(back.lines[0].spans[0]
        .style
        .add_modifier
        .contains(Modifier::BOLD));
    assert_eq!(back.lines[0].spans[1].style, Style::default());
    assert_eq!(back.lines[0].spans[2].style.fg, Some(Color::Indexed(208)));
    assert_eq!(back.lines[2].spans[0].style.fg, Some(Color::DarkGray));
}

#[test]
fn clock_and_seconds() {
    assert_eq!(clock("2026-11-16T15:04:05.123Z"), "15:04 UTC");
    assert_eq!(clock("t"), "");
    assert_eq!(seconds(1234), "1.2s");
    assert_eq!(seconds(80), "80ms");
}
