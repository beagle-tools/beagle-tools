//! Codex's stream-json, mapped onto the same tags Claude's events use, so a
//! transcript reads the same whichever runner produced it. Split from
//! [`crate::transcript`], which owns the shared shape.

use serde_json::Value;

use crate::transcript::{cap, compact, str_field, PrettyEvent};
use crate::types::Event;

/// One completed codex item: a message, a shell command (+ its output), or
/// reasoning — mapped onto the same tags the Claude events use.
pub(crate) fn item_entries(event: &Event, json: &Value) -> Vec<PrettyEvent> {
    let entry = |tag: &str, body: String| PrettyEvent {
        source: None,
        ts: event.ts,
        tag: tag.to_owned(),
        body: cap(&body),
    };
    let Some(item) = json.get("item") else {
        return vec![entry("item", compact(json))];
    };
    match str_field(item, "type").unwrap_or_default() {
        "agent_message" => vec![entry(
            "assistant",
            str_field(item, "text").unwrap_or_default().to_owned(),
        )],
        "reasoning" => match str_field(item, "text").filter(|t| !t.is_empty()) {
            Some(text) => vec![entry("thinking", text.to_owned())],
            None => Vec::new(),
        },
        "command_execution" => {
            let mut out = vec![entry(
                "tool_use",
                str_field(item, "command").unwrap_or_default().to_owned(),
            )];
            let output = str_field(item, "aggregated_output").unwrap_or_default();
            let exit = item.get("exit_code").and_then(Value::as_i64).unwrap_or(0);
            if !output.is_empty() || exit != 0 {
                let mut body = output.trim_end().to_owned();
                if exit != 0 {
                    body = format!("exit {exit} · {body}");
                }
                out.push(entry("tool_result", body));
            }
            out
        }
        other => vec![entry(
            if other.is_empty() { "item" } else { other },
            compact(item),
        )],
    }
}

/// End-of-turn summary: token usage, styled like Claude's result line.
pub(crate) fn turn_entries(event: &Event, json: &Value) -> Vec<PrettyEvent> {
    let tokens = |key: &str| {
        json.pointer(&format!("/usage/{key}"))
            .and_then(Value::as_i64)
            .unwrap_or(0)
    };
    vec![PrettyEvent {
        source: None,
        ts: event.ts,
        tag: "result".to_owned(),
        body: format!(
            "turn completed · {} in / {} out tokens",
            tokens("input_tokens"),
            tokens("output_tokens")
        ),
    }]
}
