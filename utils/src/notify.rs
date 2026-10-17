//! Helpers for desktop notifications.

/// Escapes `s` for embedding in an `AppleScript` (`osascript -e`) double-quoted
/// string literal: backslashes first, then double quotes.
///
/// Newlines and other characters pass through unchanged; `AppleScript` string
/// literals accept them as-is.
pub fn applescript_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_unchanged() {
        assert_eq!(applescript_escape("build done"), "build done");
    }

    #[test]
    fn escapes_quotes_and_backslashes() {
        assert_eq!(applescript_escape(r#"say "hi""#), r#"say \"hi\""#);
        assert_eq!(applescript_escape(r"C:\dir"), r"C:\\dir");
    }

    #[test]
    fn backslash_before_quote_is_not_double_escaped() {
        // Input `\"` must become `\\\"` (escaped backslash, escaped quote).
        assert_eq!(applescript_escape(r#"\""#), r#"\\\""#);
    }
}
