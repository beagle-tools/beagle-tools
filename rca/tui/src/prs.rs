//! Merge-status tracking for PRs attached to workspaces.
//!
//! Status comes from the `gh` CLI on a background thread — the TUI thread
//! never blocks on the network. When `gh` is missing or unauthenticated the
//! poller simply exits and attached PRs render as plain links: degraded,
//! never broken.

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

pub use beagle_utils::gh::PrState;

/// How often the poller re-queries `gh` for PR states.
pub const POLL_INTERVAL: Duration = Duration::from_secs(5 * 60);

/// One-cell status glyph for a PR state.
#[must_use]
pub fn glyph(state: PrState) -> &'static str {
    match state {
        PrState::Open => "○",
        PrState::Draft => "◌",
        PrState::Merged => "✓",
        PrState::Closed => "✗",
    }
}

/// Short display form of a PR URL: `#123` for anything with a `/pull/123`
/// path, otherwise the URL truncated to a status-line-friendly length.
#[must_use]
pub fn short_label(url: &str) -> String {
    if let Some(rest) = url.split("/pull/").nth(1) {
        let number: String = rest.chars().take_while(char::is_ascii_digit).collect();
        if !number.is_empty() {
            return format!("#{number}");
        }
    }
    let mut short: String = url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .chars()
        .take(32)
        .collect();
    if short.len() < url.trim_start_matches("https://").len() {
        short.push('…');
    }
    short
}

/// Bound on one `gh pr view` poll.
const GH_POLL_TIMEOUT: Duration = Duration::from_secs(20);

/// Queries one PR's state via `gh`. `None` on any failure — a PR whose
/// state can't be determined renders as a plain link. A hung gh skips this
/// PR for the round; the last state stays shown.
#[must_use]
pub fn state_of(url: &str) -> Option<PrState> {
    beagle_utils::gh::pr_state(url, None, GH_POLL_TIMEOUT).ok()
}

/// Whether the `gh` CLI is installed and runnable.
#[must_use]
pub fn gh_available() -> bool {
    beagle_utils::gh::available(GH_POLL_TIMEOUT)
}

/// Spawns the background poller. It receives URL-set updates on `urls_rx`
/// (poll immediately on every update), re-polls the last set every
/// [`POLL_INTERVAL`], and sends `url → state` maps back on `results_tx`.
/// Exits quietly when `gh` is unavailable or either channel closes.
#[allow(clippy::implicit_hasher)] // a channel payload, not a generic API
pub fn spawn_poller(urls_rx: Receiver<Vec<String>>, results_tx: Sender<HashMap<String, PrState>>) {
    thread::spawn(move || {
        if !gh_available() {
            return;
        }
        let mut urls: Vec<String> = Vec::new();
        loop {
            match urls_rx.recv_timeout(POLL_INTERVAL) {
                Ok(update) => urls = update,
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
            if urls.is_empty() {
                continue;
            }
            let states: HashMap<String, PrState> = urls
                .iter()
                .filter_map(|url| state_of(url).map(|state| (url.clone(), state)))
                .collect();
            if results_tx.send(states).is_err() {
                return;
            }
        }
    });
}

#[cfg(test)]
#[path = "tests/prs.rs"]
mod tests;
