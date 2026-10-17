//! Row → domain-type mapping for [`crate::state::Store`] queries.

use std::path::PathBuf;
use std::str::FromStr;

use chrono::{DateTime, Utc};
use rusqlite::Row;

use crate::types::{Event, Run};
use crate::CoreError;

pub(crate) fn run_from_row(row: &Row<'_>) -> rusqlite::Result<Run> {
    Ok(Run {
        id: row.get(0)?,
        project: row.get(1)?,
        agent: row.get(2)?,
        state: parse_col(row, 3)?,
        trigger: parse_col(row, 4)?,
        parent_run_id: row.get(5)?,
        params: row.get(6)?,
        tmux_session: row.get(7)?,
        log_path: row.get::<_, Option<String>>(8)?.map(PathBuf::from),
        workdir: row.get::<_, Option<String>>(9)?.map(PathBuf::from),
        created_at: parse_time(row, 10)?,
        started_at: parse_opt_time(row, 11)?,
        finished_at: parse_opt_time(row, 12)?,
        exit_code: row.get(13)?,
        model: row.get(14)?,
        cost_usd: row.get(15)?,
        num_turns: row.get(16)?,
        usage_json: row.get(17)?,
    })
}

pub(crate) fn event_from_row(row: &Row<'_>) -> rusqlite::Result<Event> {
    Ok(Event {
        id: row.get(0)?,
        run_id: row.get(1)?,
        ts: parse_time(row, 2)?,
        kind: row.get(3)?,
        payload: row.get(4)?,
    })
}

fn parse_col<T: FromStr<Err = CoreError>>(row: &Row<'_>, idx: usize) -> rusqlite::Result<T> {
    let text: String = row.get(idx)?;
    text.parse()
        .map_err(|e: CoreError| bad_column(idx, e.to_string()))
}

fn parse_time(row: &Row<'_>, idx: usize) -> rusqlite::Result<DateTime<Utc>> {
    let text: String = row.get(idx)?;
    DateTime::parse_from_rfc3339(&text)
        .map(|t| t.with_timezone(&Utc))
        .map_err(|e| bad_column(idx, e.to_string()))
}

fn parse_opt_time(row: &Row<'_>, idx: usize) -> rusqlite::Result<Option<DateTime<Utc>>> {
    match row.get::<_, Option<String>>(idx)? {
        None => Ok(None),
        Some(text) => DateTime::parse_from_rfc3339(&text)
            .map(|t| Some(t.with_timezone(&Utc)))
            .map_err(|e| bad_column(idx, e.to_string())),
    }
}

fn bad_column(idx: usize, message: String) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(idx, rusqlite::types::Type::Text, message.into())
}

pub(crate) fn collect_runs(
    rows: impl Iterator<Item = rusqlite::Result<Run>>,
) -> Result<Vec<Run>, CoreError> {
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub(crate) fn collect_events(
    rows: impl Iterator<Item = rusqlite::Result<Event>>,
) -> Result<Vec<Event>, CoreError> {
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}
