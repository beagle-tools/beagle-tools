//! App state tests: selection, panes and reloads.

// Tests fail loudly on setup errors; unwrap/expect/panic is the clearest way.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::pane::*;

#[test]
fn session_tab_keeps_summary_and_recent_lines() {
    let timeline: Vec<String> = (0..1500).map(|i| format!("line {i}")).collect();
    let view = format!(
        "━━ session ━━\nsummary\n── timeline ──\n{}",
        timeline.join("\n")
    );
    let shown = recent(&view, "1163b504-abcd", "04-runner");
    let lines: Vec<&str> = shown.lines().collect();
    assert_eq!(&lines[..3], ["━━ session ━━", "summary", "── timeline ──"]);
    assert!(
        lines[3].contains("500 earlier lines hidden"),
        "{}",
        lines[3]
    );
    assert!(lines[3].contains("scout session show <project> 04"));
    assert_eq!(lines[4], "line 500");
    assert_eq!(*lines.last().unwrap(), "line 1499");
    assert_eq!(lines.len(), 3 + 1 + SESSION_TAIL);
    let short = "── timeline ──\na\nb";
    assert_eq!(
        recent(short, "s", "01-x"),
        short,
        "short views are untouched"
    );
}
