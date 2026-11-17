//! Tests for `markdown`.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::*;

fn line_text(line: &Line<'_>) -> String {
    line.spans.iter().map(|s| s.content.as_ref()).collect()
}

#[test]
fn headings_are_bold_and_stripped_of_hashes() {
    let lines = to_lines("# Title\n## Sub");
    assert_eq!(line_text(&lines[0].line), "Title");
    assert!(lines[0].line.style.add_modifier.contains(Modifier::BOLD));
    assert_eq!(line_text(&lines[1].line), "Sub");
}

#[test]
fn fences_are_preformatted_and_prose_is_not() {
    let lines = to_lines("intro\n```text\n┌─┐ box\n```\nafter");
    let pre: Vec<bool> = lines.iter().map(|l| l.pre).collect();
    assert_eq!(pre, [false, true, true, true, false]);
    assert_eq!(line_text(&lines[2].line), "│ ┌─┐ box");
}

#[test]
fn bullets_checkboxes_and_inline_markup() {
    let lines = to_lines("- [x] shipped `gateway` **fix**\n- plain");
    assert_eq!(line_text(&lines[0].line), "☑ shipped gateway fix");
    assert_eq!(line_text(&lines[1].line), "• plain");
}

#[test]
fn html_comments_are_hidden_and_unbalanced_markers_stay() {
    let lines = to_lines("<!-- agent note -->\na ** b `c");
    assert_eq!(lines.len(), 1);
    assert_eq!(line_text(&lines[0].line), "a ** b `c");
}
