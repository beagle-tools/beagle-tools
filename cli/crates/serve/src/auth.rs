//! The bearer token: its format, and checking a request's `Authorization`.

use std::fmt;

/// Hex characters in a token: 32 random bytes.
pub const TOKEN_HEX_LEN: usize = 64;

/// A valid bearer token: exactly [`TOKEN_HEX_LEN`] lowercase hex characters.
/// `Debug` never prints it.
#[derive(Clone, PartialEq, Eq)]
pub struct Token(String);

impl Token {
    /// Validates `raw` (surrounding whitespace ignored).
    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        let raw = raw.trim();
        let valid = raw.len() == TOKEN_HEX_LEN
            && raw
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        valid.then(|| Self(raw.to_owned()))
    }

    /// The token text (secret: never log it).
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether an `Authorization` header value carries this token, as
    /// `Bearer <token>`. Compared in constant time for equal lengths, so
    /// response timing does not reveal how much of a guess was right.
    #[must_use]
    pub fn authorizes(&self, header: Option<&str>) -> bool {
        let Some(presented) = header.and_then(bearer) else {
            return false;
        };
        constant_time_eq(presented.as_bytes(), self.0.as_bytes())
    }
}

impl fmt::Debug for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Token(<redacted>)")
    }
}

/// The credentials of a `Bearer` authorization (scheme case-insensitive).
fn bearer(header: &str) -> Option<&str> {
    let (scheme, rest) = header.trim().split_once(' ')?;
    scheme
        .eq_ignore_ascii_case("bearer")
        .then(|| rest.trim())
        .filter(|t| !t.is_empty())
}

/// Equal-length slices are compared without an early exit.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
// Tests fail loudly on setup errors; unwrap/expect is the clearest way.
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    const GOOD: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    #[test]
    fn parse_accepts_only_64_lowercase_hex() {
        assert!(Token::parse(GOOD).is_some());
        assert!(Token::parse(&format!(" {GOOD}\n")).is_some());
        assert!(Token::parse(&GOOD.to_uppercase()).is_none());
        assert!(Token::parse(&GOOD[1..]).is_none());
        assert!(Token::parse(&format!("{GOOD}0")).is_none());
        assert!(Token::parse(&GOOD.replace('a', "g")).is_none());
        assert!(Token::parse("").is_none());
    }

    #[test]
    fn authorizes_only_the_exact_bearer_token() {
        let token = Token::parse(GOOD).unwrap();
        assert!(token.authorizes(Some(&format!("Bearer {GOOD}"))));
        assert!(token.authorizes(Some(&format!("bearer  {GOOD} "))));
        assert!(!token.authorizes(None));
        assert!(!token.authorizes(Some(GOOD)));
        assert!(!token.authorizes(Some("Bearer ")));
        assert!(!token.authorizes(Some(&format!("Basic {GOOD}"))));
        let wrong = GOOD.replace('0', "1");
        assert!(!token.authorizes(Some(&format!("Bearer {wrong}"))));
        assert!(!token.authorizes(Some(&format!("Bearer {}", &GOOD[..10]))));
    }

    #[test]
    fn debug_never_prints_the_token() {
        let token = Token::parse(GOOD).unwrap();
        assert!(!format!("{token:?}").contains("0123"));
    }
}
