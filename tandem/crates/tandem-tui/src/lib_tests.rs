//! Tests for `add`'s idle-queue warning.
#![allow(clippy::unwrap_used)]

use chrono::{Duration, TimeZone, Utc};

use super::idle_warning;

#[test]
fn add_warns_when_no_kick_and_no_recent_reviewer() {
    let now = Utc.with_ymd_and_hms(2026, 1, 2, 12, 0, 0).unwrap();
    let warning = idle_warning(false, None, now, "beagle tandem").unwrap();
    assert!(warning.contains("no `kick` command"), "{warning}");
    assert!(warning.contains("beagle tandem claim"), "{warning}");
    assert!(warning.contains("beagle tandem app-review"), "{warning}");
    // A claim older than a day does not count as a running reviewer.
    let stale = now - Duration::hours(25);
    assert!(idle_warning(false, Some(stale), now, "tandem-cli").is_some());
}

#[test]
fn add_is_quiet_with_a_kick_or_a_recent_reviewer() {
    let now = Utc.with_ymd_and_hms(2026, 1, 2, 12, 0, 0).unwrap();
    assert!(idle_warning(true, None, now, "tandem-cli").is_none());
    let recent = now - Duration::hours(2);
    assert!(idle_warning(false, Some(recent), now, "tandem-cli").is_none());
}
