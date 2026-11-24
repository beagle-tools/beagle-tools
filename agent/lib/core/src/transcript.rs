//! Turn a run's stored events into a transcript a person can read.
//!
//! Two kinds of run, two shapes. A **model** run (Claude/codex stream-json)
//! is a conversation: thinking, tool calls and their results, a final
//! answer, and anything it was denied. A **script** run is a log: lines
//! with a level and a source — see [`crate::logtext`].
//!
//! Shared by agenttop and Agentdesk, so a run reads the same in both. Pure
//! — fully unit-testable.

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;

use beagle_utils::text::truncate_chars as truncate;

use crate::logtext::{parse_script_line, sanitize};
use crate::types::Event;

/// Shown in place of an event an older agentd stored cut in half. The
/// daemon now shrinks big events structurally (see agentd's payload.rs),
/// so this only appears on runs from before that.
const CLIPPED_NOTE: &str =
    "event too large — stored cut off by an older agentd; the full line is in the run's log file";

/// Body text caps, so one giant tool result can't drown the view.
const BODY_MAX_CHARS: usize = 1200;
const INPUT_MAX_CHARS: usize = 200;

/// One cleaned-up log entry: a short tag plus a (possibly multi-line) body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PrettyEvent {
    /// When the underlying event was recorded.
    pub ts: DateTime<Utc>,
    /// Short label for the entry kind (`tool`, `result`, a log level, ...).
    pub tag: String,
    /// Sanitised, length-capped text; may span several lines.
    pub body: String,
    /// For a script line, who said it (`gather` in `gather: wrote 6
    /// files`). Kept apart from the body so a view can colour it.
    pub source: Option<String>,
}

/// Expand one raw event into zero or more pretty entries. Noise (token
/// counters, rate-limit pings, successful hooks) is dropped; an assistant
/// message may expand into several entries (thinking, text, tool calls).
pub fn prettify(event: &Event) -> Vec<PrettyEvent> {
    let entry = |tag: &str, body: String| PrettyEvent {
        source: None,
        ts: event.ts,
        tag: tag.to_owned(),
        body: cap(&body),
    };
    let Some(payload) = event.payload.as_deref() else {
        return vec![entry(&event.kind, String::new())];
    };
    let Ok(json) = serde_json::from_str::<Value>(payload) else {
        // A JSON line that will not parse was cut off in storage by an
        // older agentd. Its kind came from the full line, so it is
        // `user`/`system`/…, not `raw` — the shape, not the kind, says so.
        if payload.trim_start().starts_with('{') {
            return vec![entry("clipped", CLIPPED_NOTE.to_owned())];
        }
        // A script's output line: a level and a source, read back out.
        if event.kind == "raw" {
            let line = parse_script_line(payload);
            return vec![PrettyEvent {
                source: line.source,
                ts: event.ts,
                tag: line.level.as_str().to_owned(),
                body: cap(&line.message),
            }];
        }
        // State changes and daemon notes.
        return vec![entry(&event.kind, payload.to_owned())];
    };
    // The marker the daemon now writes for an event too dense to shrink.
    if json.get("clipped").and_then(Value::as_bool) == Some(true) {
        return vec![entry("clipped", "event too large to store".to_owned())];
    }
    match event.kind.as_str() {
        // Claude stream-json.
        "system" => system_entries(event, &json),
        "assistant" | "user" => message_entries(event, &json),
        "result" => result_entries(event, &json),
        // Noise: Claude rate-limit pings; codex thread/turn/item lifecycle
        // markers (item.completed carries the same info with the payload).
        "rate_limit_event" | "thread.started" | "turn.started" | "item.started" => Vec::new(),
        // Codex stream-json.
        "item.completed" => crate::codex::item_entries(event, &json),
        "turn.completed" => crate::codex::turn_entries(event, &json),
        _ => vec![entry(&event.kind, compact(&json))],
    }
}

/// Plain-text rendering of a whole event list — what `y` copies.
pub fn plain_text(events: &[Event]) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    for event in events {
        for e in prettify(event) {
            // Local time, matching what the log view shows on screen.
            let ts = e.ts.with_timezone(&chrono::Local).format("%H:%M:%S");
            if e.body.contains('\n') {
                let _ = writeln!(out, "{ts} {}", e.tag);
                for line in e.body.lines() {
                    let _ = writeln!(out, "    {line}");
                }
            } else {
                let _ = writeln!(out, "{ts} {:<11} {}", e.tag, e.body);
            }
        }
    }
    out
}

fn system_entries(event: &Event, json: &Value) -> Vec<PrettyEvent> {
    let entry = |tag: &str, body: String| PrettyEvent {
        source: None,
        ts: event.ts,
        tag: tag.to_owned(),
        body: cap(&body),
    };
    match str_field(json, "subtype").unwrap_or_default() {
        "init" => {
            let cwd = str_field(json, "cwd").unwrap_or("?");
            let model = str_field(json, "model").unwrap_or("");
            let tools = json
                .get("tools")
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
            let model = if model.is_empty() {
                String::new()
            } else {
                format!(" · {model}")
            };
            vec![entry("init", format!("cwd {cwd}{model} · {tools} tools"))]
        }
        // Only failed hooks are interesting.
        "hook_response" => match json.get("exit_code").and_then(Value::as_i64) {
            Some(code) if code != 0 => {
                let name = str_field(json, "hook_name").unwrap_or("hook");
                let stderr = str_field(json, "stderr").unwrap_or("");
                vec![entry("hook", format!("{name} exit {code} {stderr}"))]
            }
            _ => Vec::new(),
        },
        "hook_started" | "thinking_tokens" => Vec::new(),
        other => vec![entry("system", other.to_owned())],
    }
}

fn message_entries(event: &Event, json: &Value) -> Vec<PrettyEvent> {
    let role = event.kind.as_str();
    let entry = |tag: &str, body: String| PrettyEvent {
        source: None,
        ts: event.ts,
        tag: tag.to_owned(),
        body: cap(&body),
    };
    let Some(blocks) = json.pointer("/message/content").and_then(Value::as_array) else {
        return vec![entry(role, compact(json))];
    };
    let mut out = Vec::new();
    for block in blocks {
        match str_field(block, "type").unwrap_or_default() {
            "text" => {
                if let Some(text) = str_field(block, "text") {
                    out.push(entry(role, text.to_owned()));
                }
            }
            "thinking" => {
                if let Some(t) = str_field(block, "thinking").filter(|t| !t.is_empty()) {
                    out.push(entry("thinking", t.to_owned()));
                }
            }
            "tool_use" => {
                let name = str_field(block, "name").unwrap_or("?");
                out.push(entry(
                    "tool_use",
                    format!("{name} {}", tool_input(block.get("input"))),
                ));
            }
            "tool_result" => out.push(entry("tool_result", tool_result_text(block))),
            _ => {}
        }
    }
    // A message whose blocks were all skipped (e.g. an empty thinking block
    // carrying only a signature) produces no entries — that's fine; falling
    // back to raw JSON here would spray noise into the view.
    out
}

fn result_entries(event: &Event, json: &Value) -> Vec<PrettyEvent> {
    let entry = |tag: &str, body: String| PrettyEvent {
        source: None,
        ts: event.ts,
        tag: tag.to_owned(),
        body: cap(&body),
    };
    let subtype = str_field(json, "subtype").unwrap_or("done");
    let secs = json
        .get("duration_ms")
        .and_then(Value::as_f64)
        .map_or_else(String::new, |ms| format!(" · {:.1}s", ms / 1000.0));
    let turns = json
        .get("num_turns")
        .and_then(Value::as_i64)
        .map_or_else(String::new, |n| {
            format!(" · {n} tool call{}", if n == 1 { "" } else { "s" })
        });
    let spend = json
        .get("total_cost_usd")
        .and_then(Value::as_f64)
        .map_or_else(String::new, |c| format!(" · ${c:.2}"));
    let text = str_field(json, "result")
        .map(|t| format!("\n{t}"))
        .unwrap_or_default();
    let mut out = vec![entry(
        "result",
        format!("{subtype}{secs}{turns}{spend}{text}"),
    )];

    // What the run tried and was refused. Buried at the end of the result
    // line it is easy to miss, and it is often the reason a run produced
    // less than it should have — so each one gets its own line.
    for denial in json
        .get("permission_denials")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let tool = str_field(denial, "tool_name").unwrap_or("tool");
        out.push(entry(
            "denied",
            format!("{tool} {}", tool_input(denial.get("tool_input"))),
        ));
    }
    out
}

/// A one-line summary of a tool's input: prefer the human field, fall back
/// to compact JSON.
fn tool_input(input: Option<&Value>) -> String {
    let Some(input) = input else {
        return String::new();
    };
    for key in ["command", "file_path", "description", "prompt", "query"] {
        if let Some(text) = str_field(input, key) {
            return truncate(text, INPUT_MAX_CHARS);
        }
    }
    truncate(&compact(input), INPUT_MAX_CHARS)
}

fn tool_result_text(block: &Value) -> String {
    match block.get("content") {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(parts)) => parts
            .iter()
            .filter_map(|p| str_field(p, "text").map(str::to_owned))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

pub(crate) fn str_field<'a>(json: &'a Value, key: &str) -> Option<&'a str> {
    json.get(key).and_then(Value::as_str)
}

pub(crate) fn compact(json: &Value) -> String {
    serde_json::to_string(json).unwrap_or_default()
}

pub(crate) fn cap(text: &str) -> String {
    truncate(&sanitize(text), BODY_MAX_CHARS)
}

#[cfg(test)]
// Tests should fail loudly on a broken fixture; unwrapping is the clearest way.
#[allow(clippy::unwrap_used)]
#[path = "tests/transcript.rs"]
mod tests;
