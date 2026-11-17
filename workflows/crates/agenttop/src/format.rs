//! Pure formatting helpers — no terminal, no store, fully unit-testable.

use chrono::{DateTime, Utc};

pub use beagle_utils::duration::human_duration;

/// How long ago, in its largest unit only — `30s`, `59m`, `19h`, `8d`. For
/// the jobs list, where `8d19h59m` pushed the row past the pane.
pub fn ago_coarse(start: DateTime<Utc>, end: DateTime<Utc>) -> String {
    match (end - start).num_seconds().max(0) {
        s if s < 60 => format!("{s}s"),
        s if s < 3_600 => format!("{}m", s / 60),
        s if s < 86_400 => format!("{}h", s / 3_600),
        s => format!("{}d", s / 86_400),
    }
}

/// Human duration between two instants (clamped at zero).
pub fn elapsed_between(start: DateTime<Utc>, end: DateTime<Utc>) -> String {
    human_duration((end - start).num_seconds())
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
    fn ago_coarse_keeps_the_largest_unit() {
        let a = Utc.with_ymd_and_hms(2026, 8, 12, 10, 0, 0).unwrap();
        let b = Utc.with_ymd_and_hms(2026, 8, 20, 5, 59, 0).unwrap();
        assert_eq!(ago_coarse(a, b), "7d");
        assert_eq!(ago_coarse(b, a), "0s");
    }

    #[test]
    fn elapsed_between_clamps_negative() {
        let a = Utc.with_ymd_and_hms(2026, 8, 12, 10, 0, 0).unwrap();
        let b = Utc.with_ymd_and_hms(2026, 8, 12, 10, 1, 30).unwrap();
        assert_eq!(elapsed_between(a, b), "1m");
        assert_eq!(elapsed_between(b, a), "0s");
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
