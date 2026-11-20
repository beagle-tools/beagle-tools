//! Finding URLs in text and opening them (tickets, PRs, dashboards) in the
//! default browser.

use std::process::Command;
use std::time::Duration;

use beagle_utils::process::spawn_bounded;

use crate::error::ToolError;

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

/// Opens `url` (or a file path) with `open` (macOS) or `xdg-open`.
/// Fire-and-forget; the child is detached from our stdio so it can never
/// corrupt the terminal.
///
/// # Errors
/// [`ToolError`] when the opener can't be spawned.
pub fn open_url(url: &str) -> Result<(), ToolError> {
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    // Detached but reaped (and killed if it hangs past 10 s).
    spawn_bounded(Command::new(opener).arg(url), Duration::from_secs(10)).map_err(|e| ToolError {
        tool: opener,
        message: e.to_string(),
    })
}

#[cfg(test)]
#[path = "tests/links.rs"]
mod tests;
