//! `beagle serve pair`: what the phone scans — `beagle://pair?url=…&token=…`
//! as a terminal QR code — and the token behind it.

use qrcode::render::unicode::Dense1x2;
use qrcode::QrCode;

use crate::auth::Token;
use crate::config::Config;
use crate::error::{Error, Result};

/// The server's address as the phone reaches it.
#[must_use]
pub fn server_url(host: &str, port: u16) -> String {
    format!("https://{host}:{port}")
}

/// The pairing link the app's QR scanner understands.
#[must_use]
pub fn pair_link(url: &str, token: &Token) -> String {
    format!(
        "beagle://pair?url={}&token={}",
        percent_encode(url),
        percent_encode(token.as_str())
    )
}

const HEX_UPPER: &[u8; 16] = b"0123456789ABCDEF";

/// RFC 3986 percent-encoding: everything but the unreserved characters
/// (`A-Z a-z 0-9 - . _ ~`) becomes `%XX`, so the value is safe in a query.
#[must_use]
pub fn percent_encode(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for b in raw.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(b));
        } else {
            out.push('%');
            out.push(char::from(HEX_UPPER[usize::from(b >> 4)]));
            out.push(char::from(HEX_UPPER[usize::from(b & 0xf)]));
        }
    }
    out
}

/// Black on white, whatever the terminal's theme: a scanner needs dark
/// modules on a light background, and a dark theme would invert them.
const BLACK_ON_WHITE: &str = "\x1b[38;5;16;48;5;231m";
const RESET: &str = "\x1b[0m";

/// `data` as a QR code drawn with Unicode half blocks (two modules per
/// character row, `█` `▀` `▄` for dark), with the standard quiet zone, each
/// line coloured black on white.
///
/// # Errors
/// [`Error::Usage`] if the data cannot fit in a QR code (it always does
/// for a pairing link).
pub fn qr(data: &str) -> Result<String> {
    let code = QrCode::new(data.as_bytes()).map_err(|e| Error::Usage(format!("QR code: {e}")))?;
    let drawn = code
        .render::<Dense1x2>()
        .dark_color(Dense1x2::Dark)
        .light_color(Dense1x2::Light)
        .quiet_zone(true)
        .build();
    Ok(drawn
        .lines()
        .map(|line| format!("{BLACK_ON_WHITE}{line}{RESET}"))
        .collect::<Vec<_>>()
        .join("\n"))
}

/// The token to pair with: the configured one, or a new one when there is
/// none (or it is malformed) or `rotate` asks for it. Returns the token
/// and whether it is new.
///
/// # Errors
/// As [`Token::generate`].
pub fn token_for(config: &Config, rotate: bool) -> Result<(Token, bool)> {
    if !rotate {
        if let Some(token) = config.token.as_deref().and_then(Token::parse) {
            return Ok((token, false));
        }
    }
    Ok((Token::generate()?, true))
}

#[cfg(test)]
#[path = "tests/pair.rs"]
mod tests;
