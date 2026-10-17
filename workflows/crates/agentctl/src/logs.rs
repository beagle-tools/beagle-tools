//! `agentctl logs` — print or follow a run's event stream.

use std::thread;
use std::time::Duration;

use agent_core::state::Store;
use agent_core::types::Event;
use anyhow::{anyhow, Result};
use chrono::Local;

use crate::fmt;
use crate::run_cmd::parse_qualified;

const PAYLOAD_MAX: usize = 120;
const BATCH: u32 = 500;
const POLL: Duration = Duration::from_secs(1);

/// Print a run's events; with `follow`, poll every second until the run
/// reaches a terminal state, then print the final state and exit code.
pub fn logs(store: &Store, target: &str, follow: bool, kind: Option<&str>) -> Result<()> {
    let run_id = resolve_target(store, target)?;
    let mut last_id = 0_i64;
    loop {
        drain(store, &run_id, &mut last_id, kind)?;
        let run = store
            .get_run(&run_id)?
            .ok_or_else(|| anyhow!("run {run_id} disappeared from the store"))?;
        if run.state.is_terminal() {
            drain(store, &run_id, &mut last_id, kind)?;
            if follow {
                let exit = run
                    .exit_code
                    .map_or_else(|| "none".to_owned(), |c| c.to_string());
                println!("run {run_id} finished: {} (exit code {exit})", run.state);
            }
            return Ok(());
        }
        if !follow {
            return Ok(());
        }
        thread::sleep(POLL);
    }
}

/// Resolve a logs target: `<project>/<agent>` means that agent's latest run,
/// anything else is treated as a run id.
fn resolve_target(store: &Store, target: &str) -> Result<String> {
    if let Some((project, agent)) = parse_qualified(target) {
        let run = store
            .latest_run(project, agent)?
            .ok_or_else(|| anyhow!("no runs recorded for agent {project}/{agent}"))?;
        Ok(run.id)
    } else {
        store
            .get_run(target)?
            .map(|run| run.id)
            .ok_or_else(|| anyhow!("run {target} not found"))
    }
}

fn drain(store: &Store, run_id: &str, last_id: &mut i64, kind: Option<&str>) -> Result<()> {
    loop {
        let batch = store.events_after(run_id, *last_id, BATCH)?;
        if batch.is_empty() {
            return Ok(());
        }
        for event in batch {
            *last_id = event.id;
            if kind.is_none_or(|k| k == event.kind) {
                println!("{}", format_event(&event));
            }
        }
    }
}

/// "HH:MM:SS  KIND  payload-summary" (local time, kind padded for alignment).
fn format_event(event: &Event) -> String {
    let time = event.ts.with_timezone(&Local).format("%H:%M:%S");
    let kind = &event.kind;
    let summary = summarize_payload(event.payload.as_deref());
    format!("{time}  {kind:<9}  {summary}")
        .trim_end()
        .to_owned()
}

/// Reduce a payload to one short line: for JSON, prefer a human-readable text
/// field, falling back to the compacted payload; raw lines are collapsed and
/// truncated.
fn summarize_payload(payload: Option<&str>) -> String {
    let Some(payload) = payload else {
        return String::new();
    };
    let text = serde_json::from_str::<serde_json::Value>(payload)
        .ok()
        .and_then(|value| human_text(&value))
        .unwrap_or_else(|| payload.to_owned());
    fmt::single_line_truncated(&text, PAYLOAD_MAX)
}

fn human_text(value: &serde_json::Value) -> Option<String> {
    if let Some(s) = value.as_str() {
        return Some(s.to_owned());
    }
    let object = value.as_object()?;
    ["text", "message", "summary", "content", "name", "tool"]
        .iter()
        .find_map(|key| object.get(*key).and_then(serde_json::Value::as_str))
        .map(str::to_owned)
}

#[cfg(test)]
// Tests should fail loudly on a broken fixture; unwrapping is the clearest way.
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use agent_core::types::Trigger;

    fn temp_store(tag: &str) -> Store {
        let path =
            std::env::temp_dir().join(format!("agentctl-logs-{tag}-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        Store::open(&path).unwrap()
    }

    #[test]
    fn payload_summaries() {
        assert_eq!(summarize_payload(None), "");
        assert_eq!(
            summarize_payload(Some("plain\nmulti line")),
            "plain multi line"
        );
        assert_eq!(
            summarize_payload(Some(r#"{"text":"hello world"}"#)),
            "hello world"
        );
        assert_eq!(summarize_payload(Some(r#"{"name":"Bash"}"#)), "Bash");
        // JSON without a human field falls back to the payload itself.
        let json = format!(r#"{{"input":"{}"}}"#, "x".repeat(200));
        let summary = summarize_payload(Some(&json));
        assert!(summary.ends_with("..."));
        assert_eq!(summary.chars().count(), PAYLOAD_MAX + 3);
        // A bare JSON string is used directly.
        assert_eq!(summarize_payload(Some(r#""just text""#)), "just text");
    }

    #[test]
    fn event_lines_are_time_kind_summary() {
        let event = Event {
            id: 1,
            run_id: "r".into(),
            ts: chrono::Utc::now(),
            kind: "state".into(),
            payload: Some("running".into()),
        };
        let line = format_event(&event);
        let mut parts = line.split_whitespace();
        assert_eq!(parts.next().unwrap().len(), 8); // HH:MM:SS
        assert_eq!(parts.next(), Some("state"));
        assert_eq!(parts.next(), Some("running"));
    }

    #[test]
    fn targets_resolve_to_run_ids() {
        let store = temp_store("resolve");
        let created = store
            .create_pending_run("personal", "digest", Trigger::Manual, None, None, None)
            .unwrap();
        assert_eq!(
            resolve_target(&store, "personal/digest").unwrap(),
            created.id
        );
        assert_eq!(resolve_target(&store, &created.id).unwrap(), created.id);
        assert!(resolve_target(&store, "personal/missing")
            .unwrap_err()
            .to_string()
            .contains("no runs"));
        assert!(resolve_target(&store, "not-a-run")
            .unwrap_err()
            .to_string()
            .contains("not found"));
    }
}
