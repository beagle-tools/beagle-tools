//! Tests for `chrome`: the shared help sheet and footer.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use ratatui::backend::TestBackend;
use ratatui::Terminal;

use super::*;

const HELP: &[(&str, &str)] = &[
    ("j / k", "move"),
    ("r", "reload from disk"),
    ("q / Q", "quit"),
];

fn screen(filter: HelpFilter<'_>) -> String {
    let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
    terminal.draw(|f| draw_help(f, HELP, filter)).unwrap();
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect()
}

#[test]
fn help_lists_every_row_and_says_how_to_close() {
    let s = screen(HelpFilter::Off);
    for (keys, action) in HELP {
        assert!(s.contains(keys) && s.contains(action), "{keys}: {s}");
    }
    assert!(s.contains("any key closes"));
    assert!(!s.contains("f filter"));
    assert!(screen(HelpFilter::Available).contains("f filter · any key closes"));
}

#[test]
fn a_filter_narrows_the_rows_fuzzily() {
    assert_eq!(help_rows(HELP, Some("reld")), [("r", "reload from disk")]);
    assert_eq!(help_rows(HELP, Some("")).len(), HELP.len());
    assert_eq!(help_rows(HELP, None).len(), HELP.len());
    let s = screen(HelpFilter::Typing("zzz"));
    assert!(s.contains("(no matching keys)") && s.contains("filter: zzz"));
}

fn text(line: &Line<'_>) -> String {
    line.spans.iter().map(|s| s.content.as_ref()).collect()
}

#[test]
fn the_footer_shows_status_over_hints_and_the_resize_hint_over_both() {
    assert_eq!(text(&footer_line("j/k move", None, false)), " j/k move");
    assert_eq!(
        text(&footer_line("j/k move", Some(("saved", Tone::Ok)), false)),
        " ✓ saved"
    );
    assert_eq!(
        text(&footer_line(
            "j/k move",
            Some(("push failed", Tone::Error)),
            false
        )),
        " ✗ push failed"
    );
    assert_eq!(
        text(&footer_line("x", Some(("syncing", Tone::Busy)), false)),
        " ⟳ syncing"
    );
    let resizing = text(&footer_line("j/k move", Some(("widest", Tone::Info)), true));
    assert!(resizing.starts_with(&format!(" {}", pane_resize::HINT)));
    assert!(resizing.ends_with("   widest"));
}
