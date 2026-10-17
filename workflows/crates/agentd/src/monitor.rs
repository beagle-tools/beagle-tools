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
use chrono::{Duration, Utc};
use tracing::{info, warn};

use crate::{notify, tmux};

/// Marker line the run script appends to the log with the runner's exit code.
const EXIT_MARKER: &str = "__AGENTD_EXIT__:";
/// Event payloads are capped at this many bytes.
/// How many bytes of log tail to scan for the exit marker.
const TAIL_BYTES: u64 = 4096;
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

/// Persist a run's self-authored announcement (`$AGENTD_DATA_DIR/notify.txt`,
/// e.g. "auth failed — run the cloud login") as a `note` event, so the reason
/// shows in agenttop's activity column and log view, not just the banner.
fn record_announcement(store: &Store, run: &Run) {
    let Ok(dir) = paths::data_dir() else { return };
    let Ok(text) = std::fs::read_to_string(dir.join(&run.id).join("notify.txt")) else {
        return;
    };
    let trimmed = text.trim();
    if !trimmed.is_empty() {
        let _ = store.insert_event(&run.id, "note", Some(trimmed));
    }
}

/// Where ingestion should start for a run first seen by this daemon process:
/// byte 0 for fresh runs, end-of-file when the log was already ingested by a
/// previous daemon (the run has log-derived events, not just daemon ones).
fn initial_offset(store: &Store, run_id: &str, path: &Path) -> Result<u64> {
    let events = store.events_after(run_id, 0, 100)?;
    let has_log_events = events.iter().any(|e| e.kind != "state" && e.kind != "note");
    if has_log_events {
        Ok(std::fs::metadata(path).map_or(0, |m| m.len()))
    } else {
        Ok(0)
    }
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
        Command::new("git")
            .arg("-C")
            .arg(&spec.workdir)
            .args(["worktree", "remove"])
            .arg(workdir)
            .output()
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
mod tests {
    use super::*;

    #[test]
    fn exit_marker_parsing() {
        assert_eq!(parse_exit_marker("__AGENTD_EXIT__:0\n"), Some(0));
        assert_eq!(parse_exit_marker("noise\n__AGENTD_EXIT__:17\n"), Some(17));
        // The last marker wins.
        assert_eq!(
            parse_exit_marker("__AGENTD_EXIT__:1\n__AGENTD_EXIT__:0\n"),
            Some(0)
        );
        assert_eq!(parse_exit_marker("no marker here\n"), None);
        assert_eq!(parse_exit_marker("__AGENTD_EXIT__:not-a-number\n"), None);
        assert_eq!(parse_exit_marker(""), None);
    }

    #[test]
    fn final_state_from_exit_code() {
        assert_eq!(final_state(Some(0)), RunState::Succeeded);
        assert_eq!(final_state(Some(2)), RunState::Failed);
        assert_eq!(final_state(None), RunState::Failed);
    }

    #[test]
    fn classify_json_lines_by_type_field() {
        let (kind, payload) = classify_line(r#"{"type":"tool_use","name":"Bash"}"#).unwrap();
        assert_eq!(kind, "tool_use");
        assert_eq!(payload, r#"{"type":"tool_use","name":"Bash"}"#);
    }

    #[test]
    fn classify_non_json_and_typeless_json_as_raw() {
        assert_eq!(classify_line("plain text output").unwrap().0, "raw");
        assert_eq!(classify_line(r#"{"no_type":true}"#).unwrap().0, "raw");
        assert_eq!(classify_line(r#"["an","array"]"#).unwrap().0, "raw");
        assert_eq!(classify_line(r#"{"type":42}"#).unwrap().0, "raw");
    }

    #[test]
    fn classify_skips_marker_and_blank_lines() {
        assert_eq!(classify_line("__AGENTD_EXIT__:0"), None);
        assert_eq!(classify_line(""), None);
        assert_eq!(classify_line("\r"), None);
    }

    #[test]
    fn classify_keeps_long_json_payloads_valid() {
        // This test used to assert the payload was cut to exactly the cap
        // — i.e. that a long JSON line was stored as broken JSON. What the
        // views need is the opposite: under the cap AND still parseable.
        let long = format!(r#"{{"type":"message","body":"{}"}}"#, "x".repeat(8000));
        let (kind, payload) = classify_line(&long).unwrap();
        assert_eq!(kind, "message");
        assert!(payload.len() <= crate::payload::MAX_PAYLOAD_BYTES);
        let json: serde_json::Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(json["type"], "message");
    }

    #[test]
    fn complete_lines_keeps_trailing_partial() {
        let (lines, consumed) = complete_lines(b"one\ntwo\npart");
        assert_eq!(lines, vec!["one".to_owned(), "two".to_owned()]);
        assert_eq!(consumed, 8);

        let (lines, consumed) = complete_lines(b"no newline");
        assert_eq!(lines, [] as [std::string::String; 0]);
        assert_eq!(consumed, 0);

        let (lines, consumed) = complete_lines(b"done\n");
        assert_eq!(lines, vec!["done".to_owned()]);
        assert_eq!(consumed, 5);
    }
}
