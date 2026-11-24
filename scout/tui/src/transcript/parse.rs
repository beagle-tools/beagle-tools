//! Claude transcript lines into scout entries.

use beagle_utils::text;

use super::{Entry, Value, MAX_RESULT, MAX_SUMMARY, MAX_TEXT};

/// Converts one transcript line into scout entries (none, for the many line
/// types that are bookkeeping).
#[must_use]
pub fn parse_line(line: &str) -> Vec<Entry> {
    let Ok(v) = serde_json::from_str::<Value>(line) else {
        return Vec::new();
    };
    // Subagents write their own transcripts; meta lines are injected text
    // (skill bodies, caveats) the user never typed.
    if flag(&v, "isSidechain") || flag(&v, "isMeta") {
        return Vec::new();
    }
    let ts = text(&v, "timestamp").unwrap_or_default();
    match v.get("type").and_then(Value::as_str) {
        Some("user") => user(&v, ts),
        Some("assistant") => assistant(&v, &ts),
        _ => Vec::new(),
    }
}

pub(super) fn user(v: &Value, ts: String) -> Vec<Entry> {
    let content = &v["message"]["content"];
    if let Some(prompt) = content.as_str() {
        let origin = v["origin"]["kind"].as_str();
        return match origin {
            Some("task-notification") => vec![Entry::Notice {
                ts,
                text: notice_text(prompt),
            }],
            Some("human") | None => prompt_entry(ts, prompt, text(v, "cwd"))
                .into_iter()
                .collect(),
            Some(_) => Vec::new(),
        };
    }
    let Some(blocks) = content.as_array() else {
        return Vec::new();
    };
    let details = v.get("toolUseResult");
    let mut entries = Vec::new();
    for b in blocks.iter().filter(|b| b["type"] == "tool_result") {
        let id = b["tool_use_id"].as_str().unwrap_or_default().to_owned();
        let error = b["is_error"].as_bool().unwrap_or(false);
        entries.push(Entry::Result {
            ts: ts.clone(),
            id: id.clone(),
            text: cap(&without_harness_notes(&result_text(b, details)), MAX_RESULT),
            error,
        });
        if let Some(change) = details.filter(|_| !error).and_then(|d| change(d, &ts, &id)) {
            entries.push(change);
        }
    }
    entries
}

/// A file change from an Edit/Write result's details: `type: "create"`
/// with the new content, or a `structuredPatch` of hunks whose lines start
/// with `+`, `-` or a space.
pub(super) fn change(details: &Value, ts: &str, id: &str) -> Option<Entry> {
    let path = details.get("filePath")?.as_str()?.to_owned();
    let created = details["type"] == "create";
    let (added, removed) = if created {
        let lines = details["content"]
            .as_str()
            .unwrap_or_default()
            .lines()
            .count();
        (lines, 0)
    } else {
        let hunks = details.get("structuredPatch")?.as_array()?;
        let mut counts = (0, 0);
        for line in hunks
            .iter()
            .filter_map(|h| h["lines"].as_array())
            .flatten()
            .filter_map(Value::as_str)
        {
            match line.as_bytes().first() {
                Some(b'+') => counts.0 += 1,
                Some(b'-') => counts.1 += 1,
                _ => {}
            }
        }
        counts
    };
    Some(Entry::Change {
        ts: ts.to_owned(),
        id: id.to_owned(),
        path,
        created,
        added,
        removed,
    })
}

/// A typed prompt, with slash-command markup turned back into `/name args`
/// and harness-only wrappers dropped.
pub(super) fn prompt_entry(ts: String, prompt: &str, cwd: Option<String>) -> Option<Entry> {
    let trimmed = prompt.trim();
    if let Some(name) = between(trimmed, "<command-name>", "</command-name>") {
        let args = between(trimmed, "<command-args>", "</command-args>").unwrap_or_default();
        let text = format!("{name} {args}").trim().to_owned();
        return Some(Entry::Prompt { ts, text, cwd });
    }
    // Local command output, caveats and reminders are not the user talking.
    if ["<local-command", "<system-reminder", "<command-message"]
        .iter()
        .any(|p| trimmed.starts_with(p))
    {
        return None;
    }
    if trimmed.is_empty() {
        return None;
    }
    Some(Entry::Prompt {
        ts,
        text: cap(&strip_pasted_tags(trimmed), MAX_TEXT),
        cwd,
    })
}

pub(super) fn assistant(v: &Value, ts: &str) -> Vec<Entry> {
    let Some(blocks) = v["message"]["content"].as_array() else {
        return Vec::new();
    };
    let ms = v.get("thinkingDurationMs").and_then(Value::as_u64);
    blocks
        .iter()
        .filter_map(|b| match b["type"].as_str()? {
            "thinking" => Some(Entry::Thinking {
                ts: ts.to_owned(),
                text: b["thinking"]
                    .as_str()
                    .map(str::trim)
                    .filter(|t| !t.is_empty())
                    .map(|t| cap(t, MAX_TEXT)),
                ms,
            }),
            "text" => {
                let text = b["text"].as_str()?.trim();
                (!text.is_empty()).then(|| Entry::Text {
                    ts: ts.to_owned(),
                    text: cap(text, MAX_TEXT),
                })
            }
            "tool_use" => Some(tool(b, ts)),
            _ => None,
        })
        .collect()
}

pub(super) fn tool(b: &Value, ts: &str) -> Entry {
    let name = b["name"].as_str().unwrap_or("tool").to_owned();
    let input = &b["input"];
    let get = |key: &str| input[key].as_str().map(str::to_owned);
    let (summary, detail) = match name.as_str() {
        "Bash" => (get("command").unwrap_or_default(), get("description")),
        "Read" | "Write" | "Edit" | "NotebookEdit" => (
            get("file_path")
                .or_else(|| get("notebook_path"))
                .unwrap_or_default(),
            None,
        ),
        "Grep" | "Glob" => (
            get("pattern").unwrap_or_default(),
            get("path").or_else(|| get("glob")),
        ),
        "Agent" | "Task" => (get("description").unwrap_or_default(), get("subagent_type")),
        "Skill" => (
            format!(
                "/{} {}",
                get("skill").unwrap_or_default(),
                get("args").unwrap_or_default()
            )
            .trim()
            .to_owned(),
            None,
        ),
        "AskUserQuestion" => (
            input["questions"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|q| q["question"].as_str())
                .collect::<Vec<_>>()
                .join(" · "),
            None,
        ),
        "WebFetch" | "WebSearch" => (
            get("url").or_else(|| get("query")).unwrap_or_default(),
            None,
        ),
        "TodoWrite" => ("update the todo list".to_owned(), None),
        _ => (text::one_line(&input.to_string(), 160), None),
    };
    Entry::Tool {
        ts: ts.to_owned(),
        id: b["id"].as_str().unwrap_or_default().to_owned(),
        name,
        summary: cap(&summary, MAX_SUMMARY),
        detail,
    }
}

/// A tool result's text: the block content (string or text parts), or for
/// `AskUserQuestion` the answers as `question → answer` lines.
pub(super) fn result_text(block: &Value, details: Option<&Value>) -> String {
    if let Some(answers) = details
        .and_then(|d| d.get("answers"))
        .and_then(Value::as_object)
    {
        return answers
            .iter()
            .map(|(q, a)| format!("{q} → {}", a.as_str().unwrap_or_default()))
            .collect::<Vec<_>>()
            .join("\n");
    }
    match &block["content"] {
        Value::String(s) => s.clone(),
        Value::Array(parts) => parts
            .iter()
            .map(|p| match p["type"].as_str() {
                Some("text") => p["text"].as_str().unwrap_or_default().to_owned(),
                Some("image") => "[image]".to_owned(),
                _ => String::new(),
            })
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// Drops lines Claude Code itself appends to command output (not part of
/// what the command printed).
pub(super) fn without_harness_notes(text: &str) -> String {
    text.lines()
        .filter(|l| !l.starts_with("Shell cwd was reset to "))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn notice_text(raw: &str) -> String {
    between(raw, "<summary>", "</summary>")
        .or_else(|| between(raw, "<status>", "</status>"))
        .map_or_else(|| text::one_line(raw, 160), str::to_owned)
}

/// `<pasted_content …>…</pasted_content …>` wrappers become a marker line
/// around the pasted text, so the prompt reads naturally.
pub(super) fn strip_pasted_tags(text: &str) -> String {
    text.lines()
        .map(|line| {
            let t = line.trim();
            if t.starts_with("<pasted_content") {
                "┄ pasted ┄"
            } else if t.starts_with("</pasted_content") {
                "┄┄┄┄┄┄┄┄┄"
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn between<'a>(text: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let start = text.find(open)? + open.len();
    let end = text[start..].find(close)? + start;
    Some(text[start..end].trim())
}

pub(super) fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_owned)
}

pub(super) fn flag(v: &Value, key: &str) -> bool {
    v.get(key).and_then(Value::as_bool).unwrap_or(false)
}

/// Cuts to `max` bytes on a char boundary, marking the cut.
pub(super) fn cap(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_owned();
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &text[..end])
}
