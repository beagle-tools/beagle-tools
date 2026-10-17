//! When a run's write-up may be deleted.
//!
//! Split out of [`crate::retention`] to keep that file inside the 500-line
//! limit. The rules live in [`summary_verdict`], which is pure over
//! `(age, run_exists, pr)` so they can be tested without a filesystem, a
//! store, or GitHub.

use agent_core::state::Store;
use chrono::Utc;
use tracing::{info, warn};

use crate::retention::{RETENTION_PROBLEM_KEY, SUMMARY_RETENTION_HOURS};

/// What `gh` says about the PR a write-up describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PrOutcome {
    /// Still open — the work is live, so the write-up is still needed.
    Open,
    /// Merged or closed this many hours ago.
    SettledHoursAgo(i64),
    /// `gh` could not tell us. Never a reason to delete.
    Unknown,
}

/// Why a write-up is being deleted. Named so the log line says which rule
/// fired — "aged out" and "its run is gone" are different problems.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SummaryVerdict {
    Keep,
    Expired,
    Orphaned,
}

/// Decide a single write-up's fate. Pure, so the rules are testable without
/// a filesystem or a store.
///
/// Two ways to go: older than the window, or no run left that it describes.
/// The filename IS the run id, which is what makes the second rule possible —
/// a write-up in a directory we own, named after the thing it belongs to, can
/// always be matched back or found to be an orphan.
pub(crate) fn summary_verdict(
    age_hours: i64,
    run_exists: bool,
    pr: Option<PrOutcome>,
) -> SummaryVerdict {
    if !run_exists {
        return SummaryVerdict::Orphaned;
    }
    match pr {
        // Open: the work is in flight, and the write-up is the cheapest way
        // for anyone — including another agent — to pick it up, so age is
        // irrelevant until it lands.
        //
        // Unknown: never delete on a failed lookup. A gh outage or an expired
        // token would otherwise quietly destroy every write-up with a PR.
        Some(PrOutcome::Open | PrOutcome::Unknown) => SummaryVerdict::Keep,
        // Merged or closed: start the clock from then, not from when the
        // file was written. You review a PR after it lands, not before.
        Some(PrOutcome::SettledHoursAgo(h)) => {
            if h >= SUMMARY_RETENTION_HOURS {
                SummaryVerdict::Expired
            } else {
                SummaryVerdict::Keep
            }
        }
        // No PR mentioned: plain age.
        None => {
            if age_hours >= SUMMARY_RETENTION_HOURS {
                SummaryVerdict::Expired
            } else {
                SummaryVerdict::Keep
            }
        }
    }
}

/// Ask `gh` what happened to a PR.
///
/// Returns [`PrOutcome::Unknown`] on any failure — gh missing, token expired,
/// network down, PR deleted. The caller keeps the file in that case, so an
/// auth problem costs you disk, never a write-up. The error text is handed
/// back so the daemon can surface it instead of failing silently.
fn pr_outcome(url: &str, now: chrono::DateTime<Utc>) -> (PrOutcome, Option<String>) {
    let out = match std::process::Command::new("gh")
        .args(["pr", "view", url, "--json", "state,mergedAt,closedAt"])
        .output()
    {
        Ok(o) => o,
        Err(e) => return (PrOutcome::Unknown, Some(format!("gh not runnable: {e}"))),
    };
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_owned();
        return (PrOutcome::Unknown, Some(err));
    }
    let Ok(json) = serde_json::from_slice::<serde_json::Value>(&out.stdout) else {
        return (
            PrOutcome::Unknown,
            Some("gh returned unparseable json".to_owned()),
        );
    };
    let state = json
        .get("state")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    if state.eq_ignore_ascii_case("open") {
        return (PrOutcome::Open, None);
    }
    // Merged or closed: date the clock from whichever timestamp is present.
    let settled = ["mergedAt", "closedAt"]
        .iter()
        .filter_map(|k| json.get(*k).and_then(serde_json::Value::as_str))
        .filter_map(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.with_timezone(&Utc))
        .next();
    match settled {
        Some(when) => (
            PrOutcome::SettledHoursAgo(now.signed_duration_since(when).num_hours()),
            None,
        ),
        // Closed but no timestamp: treat as just-settled rather than ancient.
        None => (PrOutcome::SettledHoursAgo(0), None),
    }
}

/// Sweep `summaries/` on its own, longer clock, and drop any whose run has
/// left the store. Best-effort: a failure here must never stop a tick.
pub fn prune_summaries(store: &Store, now: chrono::DateTime<Utc>) {
    let Ok(dir) = agent_core::paths::state_dir().map(|d| d.join("summaries")) else {
        return;
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return; // nothing written yet
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        // <run-id>.md — anything else is not ours; leave it alone.
        let Some(run_id) = path
            .file_stem()
            .and_then(|s| s.to_str())
            .filter(|_| path.extension().is_some_and(|e| e == "md"))
        else {
            continue;
        };
        let age_hours = entry.metadata().and_then(|m| m.modified()).map_or(0, |m| {
            now.signed_duration_since(chrono::DateTime::<Utc>::from(m))
                .num_hours()
        });
        // A store error must not look like a missing run, or a blip would
        // delete every write-up on the machine.
        let run_exists = match store.get_run(run_id) {
            Ok(found) => found.is_some(),
            Err(e) => {
                warn!(run = %run_id, error = %e, "summary sweep: store read failed, keeping");
                continue;
            }
        };
        // Only ask GitHub about files that would otherwise be deleted. A
        // subprocess per summary per tick would be absurd; this bounds it to
        // the handful that have aged out.
        let mut pr = None;
        if age_hours >= SUMMARY_RETENTION_HOURS && run_exists {
            if let Some(url) = agent_core::links::pr_url_in_file(&path) {
                let (outcome, problem) = pr_outcome(&url, now);
                if let Some(message) = problem {
                    warn!(run = %run_id, pr = %url, error = %message,
                          "cannot reach GitHub — keeping the summary");
                    let _ = store.set_meta(
                        RETENTION_PROBLEM_KEY,
                        &format!("gh: {message} (summaries kept)"),
                    );
                }
                pr = Some(outcome);
            }
        }
        match summary_verdict(age_hours, run_exists, pr) {
            SummaryVerdict::Keep => {}
            verdict => {
                let _ = std::fs::remove_file(&path);
                info!(run = %run_id, ?verdict, "pruned run summary");
            }
        }
    }
}

#[cfg(test)]
// Tests should fail loudly on a broken fixture; unwrapping is the clearest way.
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn a_pr_still_open_keeps_its_summary_forever() {
        // The point of the whole feature: an open PR means the work is live,
        // so the write-up another agent would pick it up from must survive
        // however long review takes.
        assert_eq!(
            summary_verdict(1_000, true, Some(PrOutcome::Open)),
            SummaryVerdict::Keep
        );
    }

    #[test]
    fn a_merged_pr_starts_the_clock_from_the_merge() {
        // Written days ago, merged an hour ago: still keep. You read the
        // write-up after it lands, not before.
        assert_eq!(
            summary_verdict(500, true, Some(PrOutcome::SettledHoursAgo(1))),
            SummaryVerdict::Keep
        );
        assert_eq!(
            summary_verdict(500, true, Some(PrOutcome::SettledHoursAgo(24))),
            SummaryVerdict::Expired
        );
    }

    #[test]
    fn github_being_unreachable_never_deletes() {
        // An expired gh token must cost disk, not write-ups.
        assert_eq!(
            summary_verdict(10_000, true, Some(PrOutcome::Unknown)),
            SummaryVerdict::Keep
        );
    }

    #[test]
    fn an_orphan_goes_even_with_an_open_pr() {
        // No run left means nothing can open it, PR or not.
        assert_eq!(
            summary_verdict(0, false, Some(PrOutcome::Open)),
            SummaryVerdict::Orphaned
        );
    }

    #[test]
    fn pr_urls_are_found_and_trimmed() {
        let dir = std::env::temp_dir().join(format!("agentd-pr-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("s.md");
        std::fs::write(&f, "- PR: https://github.com/o/r/pull/12 (draft)\n").unwrap();
        assert_eq!(
            agent_core::links::pr_url_in_file(&f).as_deref(),
            Some("https://github.com/o/r/pull/12")
        );
        std::fs::write(&f, "no links here\n").unwrap();
        assert_eq!(agent_core::links::pr_url_in_file(&f), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_summary_outlives_its_logs_but_not_its_run() {
        // Fresh, run still there: keep. This is the common case and the
        // reason summaries do not live in the data dir, which is gone at 4h.
        assert_eq!(summary_verdict(5, true, None), SummaryVerdict::Keep);
        assert_eq!(summary_verdict(23, true, None), SummaryVerdict::Keep);
        // Past the window.
        assert_eq!(summary_verdict(24, true, None), SummaryVerdict::Expired);
        assert_eq!(summary_verdict(200, true, None), SummaryVerdict::Expired);
        // The run left the store: orphaned, whatever its age. Nothing can
        // ever open it again, so keeping it only grows the directory.
        assert_eq!(summary_verdict(0, false, None), SummaryVerdict::Orphaned);
        assert_eq!(summary_verdict(99, false, None), SummaryVerdict::Orphaned);
    }

    #[test]
    fn orphan_beats_age_so_the_log_says_which_rule_fired() {
        // Both conditions true: report it as orphaned, because that is the
        // more surprising fact and the one worth seeing in the log.
        assert_eq!(summary_verdict(100, false, None), SummaryVerdict::Orphaned);
    }
}
