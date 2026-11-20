//! Watch running runs: ingest their logs as events, enforce timeouts, detect
//! completion, and clean up worktrees.

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::process::Command;

use agent_core::state::Store;
use agent_core::types::{AgentSpec, Run, RunState};
use agent_core::{ids, paths};
use anyhow::Result;
use beagle_utils::process::output_with_timeout;
use chrono::{Duration, Utc};
use tracing::{info, warn};

use crate::{notify, tmux};

mod notes;
use notes::{record_announcement, record_repos};

/// Marker line the run script appends to the log with the runner's exit code.
const EXIT_MARKER: &str = "__AGENTD_EXIT__:";
/// Event payloads are capped at this many bytes.
/// How many bytes of log tail to scan for the exit marker.
const TAIL_BYTES: u64 = 4096;

/// Bound on a local repo query or worktree removal: a hang must never
/// wedge the daemon tick.
const GIT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
/// Timeout applied when a run's agent spec has been deleted mid-flight.
const FALLBACK_TIMEOUT_MINUTES: u64 = 30;

/// Per-run log ingestion state plus the monitoring pass itself.
///
/// Offsets are in-memory only: after a daemon restart, runs that already have
/// log-derived events resume ingestion from the current end of file instead of
/// re-ingesting from byte 0.
#[derive(Debug)]
pub struct Monitor {
    offsets: HashMap<String, u64>,
    notify: crate::notify::NotifyConfig,
}

impl Monitor {
    pub fn new(notify: crate::notify::NotifyConfig) -> Self {
        Self {
            offsets: HashMap::new(),
            notify,
        }
    }

    /// One monitoring pass over every running run. Also serves as the startup
    /// recovery pass: runs whose tmux session died while the daemon was down
    /// get finalized here.
    pub fn tick(&mut self, store: &Store) -> Result<()> {
        let runs = store.running_runs()?;
        for run in &runs {
            if let Err(e) = self.check_run(store, run) {
                warn!(run = %run.id, error = %format!("{e:#}"), "monitor pass error for run");
            }
        }
        self.offsets
            .retain(|id, _| runs.iter().any(|r| r.id == *id));
        Ok(())
    }

    fn check_run(&mut self, store: &Store, run: &Run) -> Result<()> {
        let spec = store.get_agent(&run.project, &run.agent)?;
        self.ingest_log(store, run)?;

        let session = run
            .tmux_session
            .clone()
            .unwrap_or_else(|| ids::tmux_session_name(&run.id));
        if run_timed_out(run, spec.as_ref()) {
            if let Err(e) = tmux::kill_session(&session) {
                warn!(run = %run.id, error = %format!("{e:#}"), "failed to kill timed-out session");
            }
            store.finish_run(&run.id, RunState::TimedOut, None)?;
            store.insert_event(&run.id, "state", Some("timedout"))?;
            record_announcement(store, run);
            record_repos(store, run);
            info!(run = %run.id, agent = %run.agent, "run finished: timedout");
            notify::run_finished(spec.as_ref(), run, RunState::TimedOut, &self.notify);
            cleanup_worktree(store, run, spec.as_ref())?;
            return Ok(());
        }

        if !tmux::has_session(&session)? {
            // Catch log lines written between the ingest above and session exit.
            self.ingest_log(store, run)?;
            let exit_code = run.log_path.as_deref().and_then(read_exit_code);
            let state = final_state(exit_code);
            store.finish_run(&run.id, state, exit_code)?;
            store.insert_event(&run.id, "state", Some(state.as_str()))?;
            record_announcement(store, run);
            record_repos(store, run);
            info!(run = %run.id, agent = %run.agent, exit_code, "run finished: {state}");
            notify::run_finished(spec.as_ref(), run, state, &self.notify);
            cleanup_worktree(store, run, spec.as_ref())?;
        }
        Ok(())
    }

    /// Ingest complete newline-terminated log lines written since the last
    /// pass into the run's event feed.
    fn ingest_log(&mut self, store: &Store, run: &Run) -> Result<()> {
        let Some(path) = run.log_path.as_deref() else {
            return Ok(());
        };
        if !self.offsets.contains_key(&run.id) {
            let initial = initial_offset(store, &run.id, path)?;
            self.offsets.insert(run.id.clone(), initial);
        }
        let offset = self.offsets.get(&run.id).copied().unwrap_or(0);

        // The runner may not have created the log yet — not an error.
        let Ok(mut file) = File::open(path) else {
            return Ok(());
        };
        if file.metadata().map_or(0, |m| m.len()) <= offset {
            return Ok(());
        }
        file.seek(SeekFrom::Start(offset))?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)?;

        let (lines, consumed) = complete_lines(&buf);
        for line in &lines {
            // The runner's own accounting, if this is its terminal line.
            if let Some(usage) = crate::usage::parse_line(line) {
                store.record_usage(
                    &run.id,
                    usage.cost_usd,
                    usage.num_turns,
                    usage.usage_json.as_deref(),
                )?;
            }
            if let Some((kind, payload)) = classify_line(line) {
                store.insert_event(&run.id, &kind, Some(&payload))?;
            }
        }
        self.offsets
            .insert(run.id.clone(), offset + consumed as u64);
        Ok(())
    }
}

/// Where ingestion should start for a run first seen by this daemon process:
/// just past the lines a previous daemon already ingested.
///
/// It used to jump to end-of-file for any run with log events, which lost
/// every line written while the daemon was restarting — including, for a
/// run that finished in that gap, its `result` line, and with it the run's
/// cost. Every ingested line is exactly one log event, so the count of those
/// says how many lines to skip.
fn initial_offset(store: &Store, run_id: &str, path: &Path) -> Result<u64> {
    let ingested = store.log_event_count(run_id)?;
    if ingested == 0 {
        return Ok(0);
    }
    let Ok(bytes) = std::fs::read(path) else {
        return Ok(0);
    };
    Ok(resume_offset(&bytes, ingested))
}

/// The byte offset just past the `ingested`-th line that ingestion turns into
/// an event — the same lines `classify_line` accepts. Past the end of the
/// complete lines if there are fewer (a log rewritten under us): never re-read.
fn resume_offset(bytes: &[u8], ingested: u64) -> u64 {
    let mut seen = 0u64;
    let mut pos = 0usize;
    while let Some(nl) = bytes[pos..].iter().position(|&b| b == b'\n') {
        let end = pos + nl + 1;
        let line = String::from_utf8_lossy(&bytes[pos..end - 1]);
        if classify_line(&line).is_some() {
            seen += 1;
            if seen == ingested {
                return end as u64;
            }
        }
        pos = end;
    }
    pos as u64
}

fn run_timed_out(run: &Run, spec: Option<&AgentSpec>) -> bool {
    let minutes = spec.map_or(FALLBACK_TIMEOUT_MINUTES, |s| s.timeout_minutes);
    let timeout = i64::try_from(minutes)
        .ok()
        .and_then(Duration::try_minutes)
        .unwrap_or(Duration::MAX);
    let started = run.started_at.unwrap_or(run.created_at);
    Utc::now().signed_duration_since(started) > timeout
}

fn final_state(exit_code: Option<i32>) -> RunState {
    match exit_code {
        Some(0) => RunState::Succeeded,
        _ => RunState::Failed,
    }
}

/// Split `buf` into complete (newline-terminated) lines. Returns the lines
/// and how many bytes they consumed; a trailing partial line is left for the
/// next pass.
fn complete_lines(buf: &[u8]) -> (Vec<String>, usize) {
    let mut lines = Vec::new();
    let mut start = 0;
    for (i, byte) in buf.iter().enumerate() {
        if *byte == b'\n' {
            lines.push(String::from_utf8_lossy(&buf[start..i]).into_owned());
            start = i + 1;
        }
    }
    (lines, start)
}

/// Map one log line to an event `(kind, payload)`. Exit-marker and blank
/// lines produce no event; JSON objects with a string `"type"` field use that
/// as the kind; everything else is `raw`.
fn classify_line(line: &str) -> Option<(String, String)> {
    let line = line.trim_end_matches('\r');
    if line.is_empty() || line.starts_with(EXIT_MARKER) {
        return None;
    }
    let kind = serde_json::from_str::<serde_json::Value>(line)
        .ok()
        .and_then(|v| {
            v.get("type")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "raw".to_owned());
    Some((kind, crate::payload::fit(line)))
}

/// Read the runner's exit code from the log's `__AGENTD_EXIT__:<code>` marker
/// line, scanning only the tail of the file.
fn read_exit_code(path: &Path) -> Option<i32> {
    let mut file = File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    file.seek(SeekFrom::Start(len.saturating_sub(TAIL_BYTES)))
        .ok()?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).ok()?;
    parse_exit_marker(&String::from_utf8_lossy(&buf))
}

/// Extract the exit code from the last marker line in `text`, if any.
fn parse_exit_marker(text: &str) -> Option<i32> {
    text.lines()
        .rev()
        .find_map(|line| line.trim().strip_prefix(EXIT_MARKER))
        .and_then(|code| code.trim().parse().ok())
}

/// If the run executed in one of our worktrees, try to remove it (never
/// forced); a dirty worktree is kept and noted as an event.
fn cleanup_worktree(store: &Store, run: &Run, spec: Option<&AgentSpec>) -> Result<()> {
    let Some(workdir) = &run.workdir else {
        return Ok(());
    };
    if !workdir.starts_with(paths::worktrees_dir()?) {
        return Ok(());
    }
    let removed = spec.is_some_and(|spec| {
        output_with_timeout(
            Command::new("git")
                .arg("-C")
                .arg(&spec.workdir)
                .args(["worktree", "remove"])
                .arg(workdir),
            GIT_TIMEOUT,
        )
        .is_ok_and(|out| out.status.success())
    });
    if !removed {
        let note = format!("worktree kept (dirty): {}", workdir.display());
        store.insert_event(&run.id, "note", Some(&note))?;
        warn!(run = %run.id, worktree = %workdir.display(), "worktree kept (remove failed)");
    }
    Ok(())
}

#[cfg(test)]
// Tests should fail loudly on a broken fixture; unwrapping is the clearest way.
#[allow(clippy::unwrap_used)]
#[path = "tests/monitor.rs"]
mod tests;
