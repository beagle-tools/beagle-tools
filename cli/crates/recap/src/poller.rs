//! Live PR status from GitHub, polled on a background thread (the same
//! design as beagle's `prs.rs`): the TUI thread never blocks on the network.
//!
//! The poller receives the set of PR URLs to watch, asks `gh` for each one's
//! state, review decision and CI rollup, and sends the results back over a
//! channel — immediately on every URL-set update, then every
//! [`POLL_INTERVAL`]. When `gh` is missing or unauthenticated it exits
//! quietly and the TUI keeps showing what's on disk: degraded, never broken.

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

use serde::Deserialize;

use crate::model::PrState;

/// How often the poller re-queries `gh`.
pub const POLL_INTERVAL: Duration = Duration::from_mins(3);

/// The `--json` fields one status query asks for.
/// Bound on one `gh pr view` status poll; a hung `gh` skips that PR this
/// round (its last status stays on screen) instead of stalling the poller.
const POLL_GH_TIMEOUT: Duration = Duration::from_secs(20);

const FIELDS: &str = "state,isDraft,reviewDecision,statusCheckRollup,updatedAt,mergedAt";

/// Where a PR's reviews stand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Review {
    /// Approved.
    Approved,
    /// Changes requested.
    ChangesRequested,
    /// Waiting on a required review.
    Required,
}

/// A summary of a PR's CI checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Checks {
    /// Checks that passed (or were neutral / skipped).
    pub passed: usize,
    /// Checks that failed, errored, timed out or were cancelled.
    pub failed: usize,
    /// Checks still running or queued.
    pub pending: usize,
}

impl Checks {
    /// Whether there are any checks at all.
    #[must_use]
    pub fn any(self) -> bool {
        self.passed + self.failed + self.pending > 0
    }
}

/// One PR's live status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrStatus {
    /// The PR's state, from GitHub's `state` + `isDraft`.
    pub state: PrState,
    /// The review decision, if GitHub reports one.
    pub review: Option<Review>,
    /// CI checks.
    pub checks: Checks,
    /// When the PR last changed (RFC 3339).
    pub updated: Option<String>,
    /// When it merged (RFC 3339), if it has.
    pub merged_at: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Raw {
    state: String,
    #[serde(default)]
    is_draft: bool,
    #[serde(default)]
    review_decision: Option<String>,
    #[serde(default)]
    status_check_rollup: Vec<RawCheck>,
    #[serde(default)]
    updated_at: Option<String>,
    #[serde(default)]
    merged_at: Option<String>,
}

/// A check run (`conclusion` + `status`) or a commit status (`state`).
#[derive(Debug, Deserialize)]
struct RawCheck {
    #[serde(default)]
    conclusion: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    state: Option<String>,
}

/// Parses `gh pr view --json state,isDraft,reviewDecision,statusCheckRollup,updatedAt`.
/// `None` when the JSON isn't that shape or the state is unknown.
#[must_use]
pub fn parse_status_json(json: &str) -> Option<PrStatus> {
    let raw: Raw = serde_json::from_str(json).ok()?;
    let state = PrState::from_github(&raw.state, raw.is_draft)?;
    let review = match raw.review_decision.as_deref() {
        Some("APPROVED") => Some(Review::Approved),
        Some("CHANGES_REQUESTED") => Some(Review::ChangesRequested),
        Some("REVIEW_REQUIRED") => Some(Review::Required),
        _ => None,
    };
    let mut checks = Checks::default();
    for c in &raw.status_check_rollup {
        let outcome = c
            .conclusion
            .as_deref()
            .filter(|s| !s.is_empty())
            .or(c.state.as_deref())
            .unwrap_or("");
        match outcome {
            "SUCCESS" | "NEUTRAL" | "SKIPPED" => checks.passed += 1,
            "FAILURE" | "ERROR" | "TIMED_OUT" | "CANCELLED" | "ACTION_REQUIRED"
            | "STARTUP_FAILURE" => checks.failed += 1,
            _ if c.status.as_deref() == Some("COMPLETED") => checks.passed += 1,
            _ => checks.pending += 1,
        }
    }
    Some(PrStatus {
        state,
        review,
        checks,
        updated: raw.updated_at,
        merged_at: raw.merged_at,
    })
}

/// Queries one PR's status via `gh`. `None` on any failure.
#[must_use]
pub fn status_of(url: &str) -> Option<PrStatus> {
    let out = crate::gh::run_gh(&["pr", "view", url, "--json", FIELDS], POLL_GH_TIMEOUT).ok()?;
    if !out.status.success() {
        return None;
    }
    parse_status_json(&String::from_utf8_lossy(&out.stdout))
}

/// Whether the `gh` CLI is installed and runnable.
#[must_use]
pub fn gh_available() -> bool {
    crate::gh::run_gh(&["--version"], POLL_GH_TIMEOUT).is_ok_and(|o| o.status.success())
}

/// Polls every URL once, in order. PRs whose status can't be read are left
/// out.
#[must_use]
pub fn poll_all(urls: &[String]) -> HashMap<String, PrStatus> {
    urls.iter()
        .filter_map(|url| status_of(url).map(|s| (url.clone(), s)))
        .collect()
}

/// Spawns the background poller. It receives URL-set updates on `urls_rx`
/// (polling immediately on each), re-polls the last set every
/// [`POLL_INTERVAL`], and sends `url → status` maps on `results_tx`. Exits
/// quietly when `gh` is unavailable or either channel closes.
#[allow(clippy::implicit_hasher)] // a channel payload, not a generic API
pub fn spawn(urls_rx: Receiver<Vec<String>>, results_tx: Sender<HashMap<String, PrStatus>>) {
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
            if results_tx.send(poll_all(&urls)).is_err() {
                return;
            }
        }
    });
}

#[cfg(test)]
#[path = "tests/poller.rs"]
mod tests;
