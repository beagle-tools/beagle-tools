use std::fmt::Write as _;
use std::path::Path;

use chrono::Utc;
use rusqlite::{params, Connection};

use crate::ids::new_run_id;
use crate::store_rows::{collect_runs, run_from_row};
use crate::types::{AgentSpec, Run, RunState, Trigger};
use crate::CoreError;

/// Filter for [`Store::list_runs`]. Default lists everything, newest first.
#[derive(Debug, Default, Clone)]
pub struct RunFilter {
    /// Only runs of agents in this project.
    pub project: Option<String>,
    /// Only runs of this agent name (combine with `project` to be exact).
    pub agent: Option<String>,
    /// Only runs currently in one of these states.
    pub states: Option<Vec<RunState>>,
    /// At most this many rows (the newest ones).
    pub limit: Option<u32>,
}

/// SQLite-backed observed state. Safe for one writer (`agentd`) and many
/// readers (`agentctl`, `agenttop`) thanks to WAL mode. Open one `Store` per
/// thread — it is not `Sync`.
#[derive(Debug)]
pub struct Store {
    pub(crate) conn: Connection,
}

impl Store {
    /// Open (creating if needed) the store at `path` and migrate its schema.
    ///
    /// Enables WAL and a 5s busy timeout so readers never block the daemon.
    pub fn open(path: &Path) -> Result<Self, CoreError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| CoreError::io(parent, e))?;
        }
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "busy_timeout", "5000")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        crate::schema::create_and_migrate(&conn)?;
        Ok(Self { conn })
    }

    /// Open the store at the default state path (see [`crate::paths::db_path`]).
    pub fn open_default() -> Result<Self, CoreError> {
        Self::open(&crate::paths::db_path()?)
    }

    // ----- agents (desired state mirror) -----

    /// Make the agents table match `specs` exactly: upsert every spec, delete
    /// rows for agents that no longer exist on disk.
    pub fn reconcile_agents(&mut self, specs: &[AgentSpec]) -> Result<(), CoreError> {
        let now = Utc::now().to_rfc3339();
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM agents", [])?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO agents (project, name, spec_json, updated_at) VALUES (?1, ?2, ?3, ?4)",
            )?;
            for spec in specs {
                let json = serde_json::to_string(spec)?;
                stmt.execute(params![spec.project, spec.name, json, now])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Every agent as of the daemon's last reconcile, ordered by project then name.
    pub fn list_agents(&self) -> Result<Vec<AgentSpec>, CoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT spec_json FROM agents ORDER BY project, name")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut specs = Vec::new();
        for json in rows {
            specs.push(serde_json::from_str(&json?)?);
        }
        Ok(specs)
    }

    /// One agent by `project`/`name`, or `None` when the daemon has not loaded it.
    pub fn get_agent(&self, project: &str, name: &str) -> Result<Option<AgentSpec>, CoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT spec_json FROM agents WHERE project = ?1 AND name = ?2")?;
        let mut rows = stmt.query_map(params![project, name], |row| row.get::<_, String>(0))?;
        match rows.next() {
            Some(json) => Ok(Some(serde_json::from_str(&json?)?)),
            None => Ok(None),
        }
    }

    // ----- runs (pod lifecycle) -----

    /// Create a run in `Pending`; the daemon picks it up on its next tick.
    /// `parent` records which run triggered this one (the DAG edge);
    /// `run_params` is a JSON object exported as `AGENTD_PARAM_*`.
    pub fn create_pending_run(
        &self,
        project: &str,
        agent: &str,
        trigger: Trigger,
        parent: Option<&str>,
        run_params: Option<&str>,
        model: Option<&str>,
    ) -> Result<Run, CoreError> {
        let run = Run {
            id: new_run_id(agent),
            project: project.to_owned(),
            agent: agent.to_owned(),
            state: RunState::Pending,
            trigger,
            parent_run_id: parent.map(str::to_owned),
            params: run_params.map(str::to_owned),
            tmux_session: None,
            log_path: None,
            workdir: None,
            model: model.map(str::to_owned),
            created_at: Utc::now(),
            started_at: None,
            finished_at: None,
            exit_code: None,
            cost_usd: None,
            num_turns: None,
            usage_json: None,
        };
        self.conn.execute(
            "INSERT INTO runs (id, project, agent, state, trigger, parent_run_id, params, model, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                run.id,
                run.project,
                run.agent,
                run.state.as_str(),
                run.trigger.as_str(),
                run.parent_run_id,
                run.params,
                run.model,
                run.created_at.to_rfc3339()
            ],
        )?;
        Ok(run)
    }

    /// Currently *running* (not pending) run count for an agent — the
    /// `max_concurrent` gate at spawn time.
    pub fn running_count(&self, project: &str, agent: &str) -> Result<u32, CoreError> {
        let count: u32 = self.conn.query_row(
            "SELECT COUNT(*) FROM runs
             WHERE project = ?1 AND agent = ?2 AND state = 'running'",
            params![project, agent],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Runs queued but not yet started.
    pub fn pending_runs(&self) -> Result<Vec<Run>, CoreError> {
        self.runs_in_state(RunState::Pending)
    }

    /// Runs the daemon has started and not yet finished.
    pub fn running_runs(&self) -> Result<Vec<Run>, CoreError> {
        self.runs_in_state(RunState::Running)
    }

    fn runs_in_state(&self, state: RunState) -> Result<Vec<Run>, CoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, project, agent, state, trigger, parent_run_id, params, tmux_session,
                    log_path, workdir, created_at, started_at, finished_at, exit_code, model, cost_usd, num_turns, usage_json
             FROM runs WHERE state = ?1 ORDER BY created_at",
        )?;
        let rows = stmt.query_map(params![state.as_str()], run_from_row)?;
        collect_runs(rows)
    }

    /// Pending + running run count for an agent — used by `Forbid` concurrency.
    pub fn active_run_count(&self, project: &str, agent: &str) -> Result<u32, CoreError> {
        let count: u32 = self.conn.query_row(
            "SELECT COUNT(*) FROM runs
             WHERE project = ?1 AND agent = ?2 AND state IN ('pending', 'running')",
            params![project, agent],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Move a pending run to Running, recording where it runs and stamping
    /// `started_at` now. Only the daemon calls this.
    pub fn mark_running(
        &self,
        id: &str,
        tmux_session: &str,
        log_path: &Path,
        workdir: &Path,
    ) -> Result<(), CoreError> {
        self.conn.execute(
            "UPDATE runs SET state = 'running', tmux_session = ?2, log_path = ?3,
                    workdir = ?4, started_at = ?5 WHERE id = ?1",
            params![
                id,
                tmux_session,
                log_path.to_string_lossy(),
                workdir.to_string_lossy(),
                Utc::now().to_rfc3339()
            ],
        )?;
        Ok(())
    }

    /// Record what a run cost, from its runner's own final event. Called
    /// during log ingestion, so it may fire more than once (codex reports a
    /// turn at a time); the last write wins, which is the complete one.
    pub fn record_usage(
        &self,
        id: &str,
        cost_usd: Option<f64>,
        num_turns: Option<i64>,
        usage_json: Option<&str>,
    ) -> Result<(), CoreError> {
        self.conn.execute(
            "UPDATE runs SET cost_usd = COALESCE(?2, cost_usd),
                             num_turns = COALESCE(?3, num_turns),
                             usage_json = COALESCE(?4, usage_json)
             WHERE id = ?1",
            params![id, cost_usd, num_turns, usage_json],
        )?;
        Ok(())
    }

    /// Move a run to its terminal `state`, stamping `finished_at` now.
    pub fn finish_run(
        &self,
        id: &str,
        state: RunState,
        exit_code: Option<i32>,
    ) -> Result<(), CoreError> {
        self.conn.execute(
            "UPDATE runs SET state = ?2, exit_code = ?3, finished_at = ?4 WHERE id = ?1",
            params![id, state.as_str(), exit_code, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    /// One run by id, or `None` when it does not exist (or was pruned).
    pub fn get_run(&self, id: &str) -> Result<Option<Run>, CoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, project, agent, state, trigger, parent_run_id, params, tmux_session,
                    log_path, workdir, created_at, started_at, finished_at, exit_code, model, cost_usd, num_turns, usage_json
             FROM runs WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id], run_from_row)?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    /// The most recent run for an agent, if any.
    pub fn latest_run(&self, project: &str, agent: &str) -> Result<Option<Run>, CoreError> {
        let runs = self.list_runs(&RunFilter {
            project: Some(project.to_owned()),
            agent: Some(agent.to_owned()),
            states: None,
            limit: Some(1),
        })?;
        Ok(runs.into_iter().next())
    }

    /// Runs matching `filter`, newest first.
    pub fn list_runs(&self, filter: &RunFilter) -> Result<Vec<Run>, CoreError> {
        let mut sql = String::from(
            "SELECT id, project, agent, state, trigger, parent_run_id, params, tmux_session,
                    log_path, workdir, created_at, started_at, finished_at, exit_code, model, cost_usd, num_turns, usage_json
             FROM runs WHERE 1=1",
        );
        let mut args: Vec<String> = Vec::new();
        if let Some(project) = &filter.project {
            args.push(project.clone());
            let _ = write!(sql, " AND project = ?{}", args.len());
        }
        if let Some(agent) = &filter.agent {
            args.push(agent.clone());
            let _ = write!(sql, " AND agent = ?{}", args.len());
        }
        if let Some(states) = &filter.states {
            let placeholders: Vec<String> = states
                .iter()
                .map(|s| {
                    args.push(s.as_str().to_owned());
                    format!("?{}", args.len())
                })
                .collect();
            let _ = write!(sql, " AND state IN ({})", placeholders.join(", "));
        }
        sql.push_str(" ORDER BY created_at DESC");
        if let Some(limit) = filter.limit {
            let _ = write!(sql, " LIMIT {limit}");
        }
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(args.iter()), run_from_row)?;
        collect_runs(rows)
    }

    // Event methods (insert_event / events_after / recent_events) live in
    // the sibling `events` module to keep this file under the size limit.

    // ----- meta -----

    /// Read a daemon metadata value (e.g. the last scheduler tick), `None` if unset.
    pub fn get_meta(&self, key: &str) -> Result<Option<String>, CoreError> {
        let mut stmt = self.conn.prepare("SELECT value FROM meta WHERE key = ?1")?;
        let mut rows = stmt.query_map(params![key], |row| row.get::<_, String>(0))?;
        match rows.next() {
            Some(value) => Ok(Some(value?)),
            None => Ok(None),
        }
    }

    /// Insert or overwrite a daemon metadata value.
    pub fn set_meta(&self, key: &str, value: &str) -> Result<(), CoreError> {
        self.conn.execute(
            "INSERT INTO meta (key, value) VALUES (?1, ?2)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }
}

#[cfg(test)]
// Tests should fail loudly on a broken fixture; unwrapping is the clearest way.
#[allow(clippy::unwrap_used)]
#[path = "tests/state.rs"]
mod tests;
