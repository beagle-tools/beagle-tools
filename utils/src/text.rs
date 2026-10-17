//! String truncation and single-line collapsing.

/// At most `max` characters (Unicode scalar values, not bytes or graphemes).
///
/// When `text` is longer, the result is its first `max - 1` characters
/// followed by `…`, so the output is exactly `max` characters. `max == 0`
/// on a non-empty string yields `"…"` (one character over the limit).
pub fn truncate_chars(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_owned()
    } else {
        let mut out: String = text.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

/// The longest prefix of `s` that is at most `max` bytes and ends on a char
/// boundary. Never appends a marker and never allocates.
pub fn truncate_bytes(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Collapses every whitespace run (including newlines) into a single space,
/// trims both ends, then applies [`truncate_chars`] with `max`.
pub fn one_line(text: &str, max: usize) -> String {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    truncate_chars(&collapsed, max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_chars_keeps_short_strings() {
        assert_eq!(truncate_chars("abc", 3), "abc");
        assert_eq!(truncate_chars("", 0), "");
    }

    #[test]
    fn truncate_chars_cuts_with_ellipsis() {
        assert_eq!(truncate_chars("abcdef", 4), "abc…");
        assert_eq!(truncate_chars("abcdef", 1), "…");
        assert_eq!(truncate_chars("abcdef", 0), "…");
    }

    #[test]
    fn truncate_chars_counts_chars_not_bytes() {
        assert_eq!(truncate_chars("héllo", 5), "héllo");
        assert_eq!(truncate_chars("日本語テキスト", 3), "日本…");
    }

    #[test]
    fn truncate_bytes_respects_char_boundaries() {
        assert_eq!(truncate_bytes("hello", 10), "hello");
        assert_eq!(truncate_bytes("hello", 3), "hel");
        // 'é' is two bytes (1..3); cutting at 2 must back off to 1.
        assert_eq!(truncate_bytes("héllo", 2), "h");
        assert_eq!(truncate_bytes("日本", 2), "");
        assert_eq!(truncate_bytes("日本", 3), "日");
    }

    #[test]
    fn one_line_collapses_whitespace() {
        assert_eq!(one_line("  a\n\tb   c  ", 80), "a b c");
        assert_eq!(one_line("", 5), "");
    }

    #[test]
    fn one_line_truncates_after_collapsing() {
        assert_eq!(one_line("a\n\nb\n\ncdef", 4), "a b…");
    }
}
