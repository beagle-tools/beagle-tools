//! Copying text to the system clipboard without adding a dependency.
//!
//! Strategy: pipe to the platform's clipboard command if one works
//! (`pbcopy`, `wl-copy`, `xclip`, `xsel`); otherwise emit an OSC 52 escape
//! sequence, which modern terminals (`iTerm2`, `kitty`, `WezTerm`,
//! `Alacritty`) turn into a clipboard write — including over SSH.

use std::io::{self, Write as _};
use std::process::Command;
use std::time::Duration;

use beagle_utils::process::output_with_input_and_timeout;

use crate::error::ToolError;

/// Clipboard commands to try, in order. The first element is the binary.
const COMMANDS: &[&[&str]] = &[
    &["pbcopy"],
    &["wl-copy"],
    &["xclip", "-selection", "clipboard"],
    &["xsel", "--clipboard", "--input"],
];

/// A clipboard write is local and instant; a tool that hangs (a stuck X
/// server) must not freeze the TUI.
const COPY_TIMEOUT: Duration = Duration::from_secs(5);

/// Copies `text` to the clipboard, returning the mechanism used (`pbcopy`,
/// …, or `osc 52`) for the status line. A missing or failing command just
/// moves on to the next; OSC 52 is the last resort.
///
/// # Errors
/// [`ToolError`] only if every command fails *and* writing the OSC 52
/// sequence to stdout fails.
pub fn copy(text: &str) -> Result<&'static str, ToolError> {
    for cmd in COMMANDS {
        if pipe_to(cmd, text) {
            return Ok(cmd[0]);
        }
    }
    osc52(text).map_err(|e| ToolError {
        tool: "clipboard",
        message: format!("no working pbcopy/wl-copy/xclip/xsel, and OSC 52 failed: {e}"),
    })?;
    Ok("osc 52")
}

/// Pipes `text` into a spawned command; false if the binary is missing,
/// fails or hangs (all mean "try the next mechanism").
fn pipe_to(cmd: &[&str], text: &str) -> bool {
    let Some((program, args)) = cmd.split_first() else {
        return false;
    };
    let mut command = Command::new(program);
    command.args(args);
    output_with_input_and_timeout(&mut command, text.as_bytes(), COPY_TIMEOUT)
        .is_ok_and(|out| out.status.success())
}

/// Writes the OSC 52 clipboard sequence to stdout. Works from inside the
/// alternate screen; the terminal interprets it rather than displaying it.
fn osc52(text: &str) -> io::Result<()> {
    let mut out = io::stdout().lock();
    write!(out, "\u{1b}]52;c;{}\u{7}", base64(text.as_bytes()))?;
    out.flush()
}

/// Standard base64 (RFC 4648, with padding). Hand-rolled because 20 lines
/// beat a dependency for a single call site.
fn base64(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let byte = |i: usize| u32::from(chunk.get(i).copied().unwrap_or(0));
        let n = (byte(0) << 16) | (byte(1) << 8) | byte(2);
        let sextet = |shift: u32| {
            // `& 63` bounds the value to 0..64, so the cast cannot truncate.
            char::from(ALPHABET[usize::from(u8::try_from((n >> shift) & 63).unwrap_or(0))])
        };
        out.push(sextet(18));
        out.push(sextet(12));
        out.push(if chunk.len() > 1 { sextet(6) } else { '=' });
        out.push(if chunk.len() > 2 { sextet(0) } else { '=' });
    }
    out
}

#[cfg(test)]
#[path = "tests/clipboard.rs"]
mod tests;
