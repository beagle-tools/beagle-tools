use chrono::Utc;

use super::*;

#[test]
fn window_follows_the_tail_at_zero_scroll() {
    assert_eq!(window(100, 10, 0), (90, 100));
    assert_eq!(window(5, 10, 0), (0, 5));
    assert_eq!(window(0, 10, 0), (0, 0));
}

#[test]
fn window_scrolls_up_and_clamps_at_the_top() {
    assert_eq!(window(100, 10, 5), (85, 95));
    assert_eq!(window(100, 10, 90), (0, 10));
    assert_eq!(window(100, 10, 500), (0, 10)); // over-scroll clamps
    assert_eq!(window(5, 10, 3), (0, 5)); // everything fits: no scroll
}

#[test]
fn highlighting_marks_every_case_insensitive_hit() {
    let line = Line::from(vec![
        Span::raw("12:00:00 "),
        Span::raw("tool_use    "),
        Span::raw("Error here and ERROR again"),
    ]);
    let out = highlight(&line, "error", false);
    let marked: Vec<&str> = out
        .spans
        .iter()
        .filter(|s| s.style.bg == Some(Color::Yellow))
        .map(|s| s.content.as_ref())
        .collect();
    // Both hits marked, original casing preserved in the output text.
    assert_eq!(marked, vec!["Error", "ERROR"]);
    let rebuilt: String = out.spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(rebuilt, "12:00:00 tool_use    Error here and ERROR again");
}

#[test]
fn the_current_hit_is_coloured_differently() {
    // If every hit looks the same, n/N appear to do nothing.
    let line = Line::from(vec![Span::raw("an error here")]);
    let others = highlight(&line, "error", false);
    let current = highlight(&line, "error", true);
    let bg = |l: &Line<'static>| {
        l.spans
            .iter()
            .find(|s| s.content.as_ref() == "error")
            .and_then(|s| s.style.bg)
    };
    assert_eq!(bg(&others), Some(Color::Yellow));
    assert_eq!(bg(&current), Some(Color::Magenta));
    assert_ne!(bg(&others), bg(&current));
}

#[test]
fn highlighting_an_empty_term_changes_nothing() {
    let line = Line::from(vec![Span::raw("untouched")]);
    assert_eq!(highlight(&line, "", false).spans.len(), 1);
}

#[test]
fn width_changes_line_indices_so_search_must_share_it() {
    // The bug this guards: search computed line indices at a hardcoded
    // width 80 while the body wrapped at the real one, so the "current"
    // hit highlighted a different row than the one it scrolled to.
    let long = "lorem ipsum dolor sit amet ".repeat(12);
    let events = vec![Event {
        id: 1,
        run_id: "r".to_owned(),
        ts: chrono::Utc::now(),
        kind: "note".to_owned(),
        payload: Some(long),
    }];
    let narrow = plain_lines(&events, 60, true).len();
    let wide = plain_lines(&events, 200, true).len();
    assert_ne!(
        narrow, wide,
        "wrapping width changes the line count, so indices are width-dependent"
    );
}

#[test]
fn body_metrics_match_the_layout() {
    // header row (1) + top and bottom border (2) = 3 rows of chrome,
    // and the borders take a column each side.
    assert_eq!(body_metrics(100, 40), (98, 37));
    // Never zero, however small the window.
    assert_eq!(body_metrics(1, 1), (0, 1));
}

#[test]
fn plain_lines_match_what_is_drawn() {
    // The indices search returns must be the rows on screen, so the two
    // must stay the same length for the same inputs.
    let events = vec![Event {
        id: 1,
        run_id: "r".to_owned(),
        ts: chrono::Utc::now(),
        kind: "note".to_owned(),
        payload: Some("alpha beta".to_owned()),
    }];
    assert_eq!(
        plain_lines(&events, 60, true).len(),
        build_lines(&events, 60, true).len()
    );
    assert!(plain_lines(&events, 60, true)[0].contains("alpha beta"));
}

#[test]
fn long_bodies_wrap_into_indented_lines() {
    let event = Event {
        id: 1,
        run_id: "r".to_owned(),
        ts: Utc::now(),
        kind: "state".to_owned(),
        payload: Some("word ".repeat(30).trim_end().to_owned()),
    };
    // Body width bottoms out at 20, so 30 five-char words need many lines.
    let lines = build_lines(std::slice::from_ref(&event), 40, true);
    assert!(lines.len() > 5);
    // Unwrapped, the same body stays on a single display line.
    assert_eq!(build_lines(&[event], 40, false).len(), 1);
    assert_eq!(line_count(&[], 40, true), 0);
}
