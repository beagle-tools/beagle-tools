//! Whether a thread needs an agent review, decided without one.

use std::path::Path;

use super::{is_read, is_reviewed, load, load_read, Thread, ThreadKind};
/// How long a thread's newest comment must sit before it is reviewed:
/// people post in bursts, and one review of the burst beats one per comment.
pub const QUIET_SECS: i64 = 10 * 60;

/// `YYYY-MM-DDTHH:MM:SSZ` (GitHub's format) as Unix seconds.
#[must_use]
pub fn unix_of(rfc3339: &str) -> Option<i64> {
    let num = |range: std::ops::Range<usize>| rfc3339.get(range)?.parse::<i64>().ok();
    let (y, mo, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (h, mi, s) = (num(11..13)?, num(14..16)?, num(17..19)?);
    // Days from the civil date (Howard Hinnant's algorithm).
    let y = if mo <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (mo + if mo > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400 + h * 3600 + mi * 60 + s)
}

/// Whether a thread needs an agent, decided without one — the strict,
/// free checks that keep the reviewer from running on what it need not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Need {
    /// Review it: someone is waiting on the PR author and no review answers
    /// them.
    Review,
    /// Someone is waiting on the author, but their newest comment is under
    /// [`QUIET_SECS`] old — let the burst finish first.
    Settling,
    /// The current review already answers the newest comment.
    Reviewed,
    /// The author spoke last: the thread is waiting on the others.
    TheirTurn,
    /// Nothing to answer: only the author, or an approval without a question.
    Nothing,
}

/// The decision for one thread at `now` (Unix seconds).
#[must_use]
pub fn need(recap_dir: &Path, thread: &Thread, pr_author: &str, now: i64) -> Need {
    let Some(last) = thread.last_counted() else {
        return Need::Nothing;
    };
    if last.author == pr_author {
        return if thread.last_from_others(pr_author).is_some() {
            Need::TheirTurn
        } else {
            Need::Nothing
        };
    }
    // An approval is informational unless it asks something.
    if let ThreadKind::Review { state } = &thread.kind {
        if state == "APPROVED" && !last.body.contains('?') {
            return Need::Nothing;
        }
    }
    if is_reviewed(recap_dir, thread, pr_author) {
        return Need::Reviewed;
    }
    match unix_of(&last.created) {
        Some(at) if now - at < QUIET_SECS => Need::Settling,
        _ => Need::Review,
    }
}

/// Per-recap counts for the PR list.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Marks {
    /// Threads with something you have not read.
    pub unread: usize,
    /// Threads the agent has reviewed (current).
    pub reviewed: usize,
    /// Threads waiting on an agent review (or about to be).
    pub pending: usize,
    /// All threads.
    pub total: usize,
}

/// The counts for one recap.
#[must_use]
pub fn marks(recap_dir: &Path, pr_author: &str, now: i64) -> Marks {
    let threads = load(recap_dir);
    let read = load_read(recap_dir);
    let mut m = Marks {
        total: threads.threads.len(),
        ..Marks::default()
    };
    for t in &threads.threads {
        if !is_read(t, &read) {
            m.unread += 1;
        }
        match need(recap_dir, t, pr_author, now) {
            Need::Review | Need::Settling => m.pending += 1,
            Need::Reviewed => m.reviewed += 1,
            Need::TheirTurn | Need::Nothing => {}
        }
    }
    m
}
