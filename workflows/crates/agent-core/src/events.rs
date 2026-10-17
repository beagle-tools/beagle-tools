//! The live event feed: per-run observations (parsed stream-json lines,
//! state changes, daemon notes). Split from `state.rs` for file size.

use chrono::Utc;
use rusqlite::params;

use crate::state::Store;
use crate::store_rows::{collect_events, event_from_row};
use crate::types::Event;
use crate::CoreError;

impl Store {
    /// Append an event to `run_id`'s feed, timestamped now. `payload` is stored
    /// as-is; callers cap its size (agentd fits runner lines to 4 KB first).
    pub fn insert_event(
        &self,
        run_id: &str,
        kind: &str,
        payload: Option<&str>,
    ) -> Result<(), CoreError> {
        self.conn.execute(
            "INSERT INTO events (run_id, ts, kind, payload) VALUES (?1, ?2, ?3, ?4)",
            params![run_id, Utc::now().to_rfc3339(), kind, payload],
        )?;
        Ok(())
    }

    /// Events for one run with id greater than `after_id` — poll with the last
    /// seen id to tail a run incrementally.
    pub fn events_after(
        &self,
        run_id: &str,
        after_id: i64,
        limit: u32,
    ) -> Result<Vec<Event>, CoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, run_id, ts, kind, payload FROM events
             WHERE run_id = ?1 AND id > ?2 ORDER BY id LIMIT ?3",
        )?;
        let rows = stmt.query_map(params![run_id, after_id, limit], event_from_row)?;
        collect_events(rows)
    }

    /// Most recent events across all runs, newest first.
    pub fn recent_events(&self, limit: u32) -> Result<Vec<Event>, CoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, run_id, ts, kind, payload FROM events ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], event_from_row)?;
        collect_events(rows)
    }
}
