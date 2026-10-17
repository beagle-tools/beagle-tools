//! POSIX shell quoting.

use std::ffi::OsStr;

/// Wraps `s` in single quotes so a POSIX shell treats it as one literal word.
///
/// Embedded single quotes are escaped as `'\''` (close quote, literal quote,
/// reopen quote); nothing else needs escaping inside single quotes, so `$`,
/// backticks, and globs stay inert. The empty string becomes `''`.
pub fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        if c == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

/// [`quote`] for an `OsStr`. Non-UTF-8 bytes are replaced with `U+FFFD`,
/// so the result is only exact for valid Unicode input.
pub fn quote_os(s: &OsStr) -> String {
    quote(&s.to_string_lossy())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_plain_strings() {
        assert_eq!(quote("abc"), "'abc'");
        assert_eq!(quote(""), "''");
        assert_eq!(quote("/a b/c.jsonl"), "'/a b/c.jsonl'");
    }

    #[test]
    fn escapes_embedded_single_quotes() {
        assert_eq!(quote("it's"), "'it'\\''s'");
        assert_eq!(quote("''"), "''\\'''\\'''");
    }

    #[test]
    fn neutralizes_shell_metacharacters() {
        let quoted = quote("$(rm -rf ~); `x` && $HOME");
        assert_eq!(quoted, "'$(rm -rf ~); `x` && $HOME'");
    }

    #[test]
    fn quote_os_matches_quote() {
        assert_eq!(quote_os(OsStr::new("it's")), quote("it's"));
    }
}
