//! The store's SQL schema and its forward-only migrations.
//!
//! Kept beside [`crate::state`] rather than inside it: the DDL is the one
//! part of the store that is read far more often than it is changed, and
//! `state.rs` is up against the 500-line limit.

use rusqlite::Connection;

use crate::CoreError;

pub(crate) const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS agents (
    project    TEXT NOT NULL,
    name       TEXT NOT NULL,
    spec_json  TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (project, name)
);
CREATE TABLE IF NOT EXISTS runs (
    id            TEXT PRIMARY KEY,
    project       TEXT NOT NULL,
    agent         TEXT NOT NULL,
    state         TEXT NOT NULL,
    trigger       TEXT NOT NULL,
    parent_run_id TEXT,
    params        TEXT,
    tmux_session  TEXT,
    log_path      TEXT,
    workdir       TEXT,
    model         TEXT,
    cost_usd      REAL,
    num_turns     INTEGER,
    usage_json    TEXT,
    created_at    TEXT NOT NULL,
    started_at    TEXT,
    finished_at   TEXT,
    exit_code     INTEGER
);
CREATE INDEX IF NOT EXISTS idx_runs_state ON runs (state);
CREATE INDEX IF NOT EXISTS idx_runs_agent ON runs (project, agent, created_at DESC);
CREATE TABLE IF NOT EXISTS events (
    id      INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id  TEXT NOT NULL,
    ts      TEXT NOT NULL,
    kind    TEXT NOT NULL,
    payload TEXT
);
CREATE INDEX IF NOT EXISTS idx_events_run ON events (run_id, id);
CREATE TABLE IF NOT EXISTS meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
";

/// Create the tables, then apply the column additions that older databases
/// predate. Each is attempted unconditionally; "duplicate column name" means
/// it is already there, which is the normal case and not an error.
pub(crate) fn create_and_migrate(conn: &Connection) -> Result<(), CoreError> {
    conn.execute_batch(SCHEMA)?;
    for ddl in [
        "ALTER TABLE runs ADD COLUMN parent_run_id TEXT",
        "ALTER TABLE runs ADD COLUMN params TEXT",
        "ALTER TABLE runs ADD COLUMN model TEXT",
        "ALTER TABLE runs ADD COLUMN cost_usd REAL",
        "ALTER TABLE runs ADD COLUMN num_turns INTEGER",
        "ALTER TABLE runs ADD COLUMN usage_json TEXT",
    ] {
        if let Err(e) = conn.execute(ddl, []) {
            if !e.to_string().contains("duplicate column name") {
                return Err(e.into());
            }
        }
    }
    Ok(())
}
