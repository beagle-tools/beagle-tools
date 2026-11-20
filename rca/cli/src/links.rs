//! URL extraction and opening, for the `o` link popup.
//!
//! Agents cite Grafana/Sentry/GitHub URLs constantly; the TUI renders links
//! as plain text, so this module finds them in raw section content and hands
//! them to the platform opener.

use std::io;
use std::process::Command;
use std::time::Duration;

use beagle_utils::process::spawn_bounded;

/// Extracts `http(s)://` URLs from free text, in order of appearance,
/// de-duplicated. URLs end at whitespace or a closing delimiter, and
/// trailing punctuation is trimmed (so `<https://x> and (https://y).`
/// both come out clean).
#[must_use]
pub fn extract_urls(text: &str) -> Vec<String> {
    let mut urls: Vec<String> = Vec::new();
    for (index, _) in text.match_indices("http") {
        let rest = &text[index..];
        if !(rest.starts_with("http://") || rest.starts_with("https://")) {
            continue;
        }
        let end = rest
            .find(|c: char| {
                c.is_whitespace() || matches!(c, '>' | ')' | ']' | '}' | '"' | '\'' | '`')
            })
            .unwrap_or(rest.len());
        let url = rest[..end].trim_end_matches(['.', ',', ';', ':', '!', '?']);
        if url.len() > "https://".len() && !urls.iter().any(|u| u == url) {
            urls.push(url.to_owned());
        }
    }
    urls
}

/// Opens a URL in the default browser: `open` on macOS, `xdg-open`
/// elsewhere. Fire-and-forget; the child is detached from our stdio so it
/// can never corrupt the terminal.
///
/// # Errors
/// Returns the spawn error when the opener is missing.
pub fn open_url(url: &str) -> io::Result<()> {
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    // Detached but reaped (and killed if it hangs past 10 s).
    spawn_bounded(Command::new(opener).arg(url), Duration::from_secs(10))
}

#[cfg(test)]
#[path = "tests/links.rs"]
mod tests;
