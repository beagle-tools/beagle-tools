//! Tests for `wrap`.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::*;
use ratatui::style::Color;

fn text(line: &Line<'_>) -> String {
    line.spans.iter().map(|s| s.content.as_ref()).collect()
}

#[test]
fn prose_wraps_at_word_boundaries() {
    let lines = [DocLine::prose(Line::from("the quick brown fox jumps over"))];
    let out: Vec<String> = layout(&lines, 10).iter().map(text).collect();
    assert_eq!(out, ["the quick", "brown fox", "jumps over"]);
}

#[test]
fn bullets_get_a_hanging_indent() {
    let lines = [DocLine::prose(Line::from("• alpha beta gamma delta"))];
    let out: Vec<String> = layout(&lines, 12).iter().map(text).collect();
    assert_eq!(out, ["• alpha beta", "  gamma", "  delta"]);
}

#[test]
fn preformatted_lines_are_never_wrapped() {
    let wide = "┌──────────────────────────┐";
    let out = layout(&[DocLine::pre(Line::from(wide))], 10);
    assert_eq!(out.len(), 1);
    assert_eq!(text(&out[0]), wide);
}

#[test]
fn long_words_are_hard_cut_and_styles_survive() {
    let red = Style::default().fg(Color::Red);
    let lines = [DocLine::prose(Line::from(vec![
        Span::raw("ab "),
        Span::styled("cdefghijklmnop", red),
    ]))];
    let out = layout(&lines, 8);
    assert_eq!(
        out.iter().map(text).collect::<Vec<_>>(),
        ["ab", "cdefghij", "klmnop"]
    );
    assert_eq!(out[1].spans[0].style, red);
}

#[test]
fn tiny_widths_do_not_panic() {
    let lines = [DocLine::prose(Line::from("x ".repeat(50)))];
    for w in 0..12 {
        let _ = layout(&lines, w);
    }
}

#[test]
fn owners_map_every_row_to_its_source_line() {
    let lines = [
        DocLine::prose(Line::from("short")),
        DocLine::prose(Line::from("the quick brown fox jumps over")),
        DocLine::pre(Line::from(vec![Span::raw("│ "), Span::raw("echo hi")])),
    ];
    let (rows, owners) = layout_with_owners(&lines, 10);
    assert_eq!(rows.len(), owners.len());
    assert_eq!(owners, [0, 1, 1, 1, 2]);
    assert_eq!(plain_text(&lines[2]), "echo hi", "code gutter is dropped");
    assert_eq!(plain_text(&lines[1]), "the quick brown fox jumps over");
}

#[test]
fn numbered_lists_get_a_hanging_indent() {
    let lines = [DocLine::prose(Line::from("2. alpha beta gamma delta"))];
    let out: Vec<String> = layout(&lines, 14).iter().map(text).collect();
    assert_eq!(out, ["2. alpha beta", "   gamma delta"]);
}
