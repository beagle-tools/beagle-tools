//! Cron scheduling: turn schedule occurrences since the last tick into
//! pending runs.

use agent_core::manifest::parse_schedule;
use agent_core::state::Store;
use agent_core::types::{AgentSpec, ConcurrencyPolicy, Trigger};
use anyhow::Result;
use chrono::{DateTime, Utc};
use cron::Schedule;
use tracing::{info, warn};

/// Meta key holding the RFC 3339 timestamp of the last completed scheduler
/// tick — the left edge of the due window.
const LAST_TICK_KEY: &str = "last_tick";

/// One scheduler pass: create pending runs for every enabled, scheduled agent
/// with an occurrence in `(last_tick, now]`, then advance `last_tick` to `now`.
///
/// On the very first tick ever there is no window, so nothing fires — the
/// baseline is just recorded.
pub fn tick(store: &Store, specs: &[AgentSpec], now: DateTime<Utc>) -> Result<()> {
    if let Some(last) = load_last_tick(store)? {
        for spec in specs {
            schedule_agent(store, spec, last, now)?;
        }
    } else {
        info!("first scheduler tick: recording baseline, firing nothing");
    }
    store.set_meta(LAST_TICK_KEY, &now.to_rfc3339())?;
    Ok(())
}

/// True when `schedule` has at least one occurrence in the window
/// `(last, now]` (exclusive of `last`, inclusive of `now`).
fn is_due(schedule: &Schedule, last: DateTime<Utc>, now: DateTime<Utc>) -> bool {
    schedule.after(&last).next().is_some_and(|t| t <= now)
}

fn schedule_agent(
    store: &Store,
    spec: &AgentSpec,
    last: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Result<()> {
    let Some(expr) = &spec.schedule else {
        return Ok(());
    };
    if !spec.enabled {
        return Ok(());
    }
    let schedule = match parse_schedule(expr) {
        Ok(schedule) => schedule,
        Err(e) => {
            warn!(agent = %spec.qualified_name(), error = %e, "skipping unparsable schedule");
            return Ok(());
        }
    };
    if !is_due(&schedule, last, now) {
        return Ok(());
    }
    if spec.concurrency == ConcurrencyPolicy::Forbid
        && store.active_run_count(&spec.project, &spec.name)? > 0
    {
        info!(
            agent = %spec.qualified_name(),
            "skipping cron tick: previous run still active (concurrency: forbid)"
        );
        return Ok(());
    }
    let run =
        store.create_pending_run(&spec.project, &spec.name, Trigger::Cron, None, None, None)?;
    info!(run = %run.id, agent = %spec.qualified_name(), "scheduled run");
    Ok(())
}

fn load_last_tick(store: &Store) -> Result<Option<DateTime<Utc>>> {
    let Some(text) = store.get_meta(LAST_TICK_KEY)? else {
        return Ok(None);
    };
    match DateTime::parse_from_rfc3339(&text) {
        Ok(t) => Ok(Some(t.with_timezone(&Utc))),
        Err(e) => {
            warn!(value = %text, error = %e, "unreadable last_tick; treating as first tick");
            Ok(None)
        }
    }
}

#[cfg(test)]
// Tests should fail loudly on a broken fixture; unwrapping is the clearest way.
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn ts(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }

    #[test]
    fn due_when_occurrence_falls_inside_window() {
        let daily_7am = parse_schedule("0 7 * * *").unwrap();
        let last = ts("2026-08-12T06:59:58Z");
        let now = ts("2026-08-12T07:00:03Z");
        assert!(is_due(&daily_7am, last, now));
    }

    #[test]
    fn not_due_when_window_misses_the_occurrence() {
        let daily_7am = parse_schedule("0 7 * * *").unwrap();
        assert!(!is_due(
            &daily_7am,
            ts("2026-08-12T07:00:01Z"),
            ts("2026-08-12T07:00:06Z")
        ));
        assert!(!is_due(
            &daily_7am,
            ts("2026-08-12T08:00:00Z"),
            ts("2026-08-12T08:00:05Z")
        ));
    }

    #[test]
    fn window_is_exclusive_of_last_and_inclusive_of_now() {
        let daily_7am = parse_schedule("0 7 * * *").unwrap();
        // Occurrence exactly at `last` fired on a previous tick: not due again.
        assert!(!is_due(
            &daily_7am,
            ts("2026-08-12T07:00:00Z"),
            ts("2026-08-12T07:00:05Z")
        ));
        // Occurrence exactly at `now` is due on this tick.
        assert!(is_due(
            &daily_7am,
            ts("2026-08-12T06:59:55Z"),
            ts("2026-08-12T07:00:00Z")
        ));
    }

    #[test]
    fn long_daemon_pause_still_counts_as_due() {
        let daily_7am = parse_schedule("0 7 * * *").unwrap();
        // Daemon slept across several occurrences; the window covers them all.
        assert!(is_due(
            &daily_7am,
            ts("2026-08-01T00:00:00Z"),
            ts("2026-08-12T12:00:00Z")
        ));
    }

    #[test]
    fn six_field_seconds_schedule_polls_subminute() {
        // The migration path for ai-pipelines' 30s poll loops: a 6-field cron
        // with a seconds column.
        let every_30s = parse_schedule("*/30 * * * * *").unwrap();
        assert!(is_due(
            &every_30s,
            ts("2026-08-12T07:00:00Z"),
            ts("2026-08-12T07:00:30Z")
        ));
        assert!(!is_due(
            &every_30s,
            ts("2026-08-12T07:00:00Z"),
            ts("2026-08-12T07:00:29Z")
        ));
    }

    #[test]
    fn every_minute_schedule_fires_each_window() {
        let every_minute = parse_schedule("* * * * *").unwrap();
        assert!(is_due(
            &every_minute,
            ts("2026-08-12T07:00:00Z"),
            ts("2026-08-12T07:01:00Z")
        ));
        assert!(!is_due(
            &every_minute,
            ts("2026-08-12T07:00:00Z"),
            ts("2026-08-12T07:00:59Z")
        ));
    }
}
