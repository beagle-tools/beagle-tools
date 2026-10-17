//! Pure formatting helpers — no terminal, no store, fully unit-testable.

use chrono::{DateTime, Utc};

pub use beagle_utils::duration::human_duration;
pub use beagle_utils::text::{one_line, truncate_chars};

/// Human duration between two instants (clamped at zero).
pub fn elapsed_between(start: DateTime<Utc>, end: DateTime<Utc>) -> String {
    human_duration((end - start).num_seconds())
}

/// Best-effort single-line snippet of an event payload.
///
/// JSON object payloads prefer a human-readable text field; anything else
/// (plain text, JSON arrays, invalid JSON) is used verbatim.
pub fn payload_snippet(payload: Option<&str>, max: usize) -> String {
    let Some(raw) = payload else {
        return String::new();
    };
    let text = json_text(raw).unwrap_or_else(|| raw.to_owned());
    one_line(&text, max)
}

fn json_text(raw: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(raw).ok()?;
    let obj = value.as_object()?;
    for key in ["text", "message", "content", "summary", "name"] {
        if let Some(s) = obj.get(key).and_then(serde_json::Value::as_str) {
            return Some(s.to_owned());
        }
    }
    None
}

/// Greedy word-wrap into lines of at most `max` characters, preserving
/// existing newlines; words longer than `max` are hard-split. Always returns
/// at least one line so empty bodies still occupy a row.
pub fn wrap_text(text: &str, max: usize) -> Vec<String> {
    let max = max.max(1);
    let mut lines = Vec::new();
    for raw in text.lines() {
        if raw.chars().count() <= max {
            lines.push(raw.to_owned());
            continue;
        }
        let mut current = String::new();
        for word in raw.split_whitespace() {
            let need = word.chars().count();
            let have = current.chars().count();
            if have == 0 && need <= max {
                current.push_str(word);
            } else if have + 1 + need <= max {
                current.push(' ');
                current.push_str(word);
            } else {
                if !current.is_empty() {
                    lines.push(std::mem::take(&mut current));
                }
                let mut chars: Vec<char> = word.chars().collect();
                while chars.len() > max {
                    lines.push(chars.drain(..max).collect());
                }
                current = chars.into_iter().collect();
            }
        }
        if !current.is_empty() {
            lines.push(current);
        }
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// Clamp a selection index so it stays within a list of `len` rows.
pub fn clamp_selection(selected: usize, len: usize) -> usize {
    if len == 0 {
        0
    } else {
        selected.min(len - 1)
    }
}

/// `$1.04` for a run's reported cost, `-` when the runner reported no price
/// (codex) or the run has not finished. Sub-cent runs keep four decimals so
/// they do not all collapse to `$0.00`.
pub fn cost(cost_usd: Option<f64>) -> String {
    match cost_usd {
        Some(c) if c >= 0.01 => format!("${c:.2}"),
        Some(c) if c > 0.0 => format!("${c:.4}"),
        Some(_) => "$0".to_owned(),
        None => "-".to_owned(),
    }
}

#[cfg(test)]
// Tests should fail loudly on a broken fixture; unwrapping is the clearest way.
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn durations_pick_the_right_unit() {
        assert_eq!(human_duration(0), "0s");
        assert_eq!(human_duration(45), "45s");
        assert_eq!(human_duration(60), "1m");
        assert_eq!(human_duration(192), "3m");
        assert_eq!(human_duration(3_600), "1h");
        assert_eq!(human_duration(3_720), "1h2m");
        assert_eq!(human_duration(2 * 86_400 + 3 * 3_600 + 20 * 60), "2d3h20m");
        assert_eq!(human_duration(86_400 + 120), "1d2m");
        assert_eq!(human_duration(-5), "0s");
    }

    #[test]
    fn elapsed_between_clamps_negative() {
        let a = Utc.with_ymd_and_hms(2026, 8, 12, 10, 0, 0).unwrap();
        let b = Utc.with_ymd_and_hms(2026, 8, 12, 10, 1, 30).unwrap();
        assert_eq!(elapsed_between(a, b), "1m");
        assert_eq!(elapsed_between(b, a), "0s");
    }

    #[test]
    fn one_line_collapses_and_truncates() {
        assert_eq!(one_line("a\nb\t  c", 10), "a b c");
        assert_eq!(one_line("hello world", 6), "hello…");
    }

    #[test]
    fn truncate_respects_char_boundaries() {
        assert_eq!(truncate_chars("héllo wörld", 6), "héllo…");
        assert_eq!(truncate_chars("short", 10), "short");
    }

    #[test]
    fn snippet_prefers_json_text_fields() {
        let json = r#"{"type":"tool_use","name":"Bash","input":{}}"#;
        assert_eq!(payload_snippet(Some(json), 40), "Bash");
        let msg = r#"{"text":"running   tests\nnow"}"#;
        assert_eq!(payload_snippet(Some(msg), 40), "running tests now");
        assert_eq!(payload_snippet(Some("plain text"), 40), "plain text");
        assert_eq!(payload_snippet(Some("[1, 2]"), 40), "[1, 2]");
        assert_eq!(payload_snippet(None, 40), "");
    }

    #[test]
    fn wrap_text_wraps_words_and_splits_long_ones() {
        assert_eq!(wrap_text("a b c", 10), vec!["a b c"]);
        assert_eq!(wrap_text("one two three", 7), vec!["one two", "three"]);
        assert_eq!(wrap_text("abcdefghij", 4), vec!["abcd", "efgh", "ij"]);
        assert_eq!(wrap_text("x\ny", 10), vec!["x", "y"]);
        assert_eq!(wrap_text("", 10), vec![String::new()]);
    }

    #[test]
    fn selection_clamps_into_range() {
        assert_eq!(clamp_selection(0, 0), 0);
        assert_eq!(clamp_selection(5, 0), 0);
        assert_eq!(clamp_selection(5, 3), 2);
        assert_eq!(clamp_selection(1, 3), 1);
    }
}
