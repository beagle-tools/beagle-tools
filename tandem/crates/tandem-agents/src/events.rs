//! Classify runner output lines into [`RunEventKind`]s, and parse the
//! `tandem_comment` contract agents use to emit review comments.
//!
//! Same approach as agentd's monitor: a line that parses as JSON with a
//! `"type"` field is a structured runner event; everything else is raw.

use serde::Deserialize;
use tandem_core::agent::RunEventKind;

/// The payload an agent emits for one review comment.
///
/// Contract (documented to the agent in its run context):
/// `{"type":"tandem_comment","path":"src/x.rs","side":"new","line":42,
///   "severity":"issue","body":"..."}`
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct AgentComment {
    /// Repo-relative file path; empty for a PR-level comment.
    pub path: String,
    /// `"old"` or `"new"`; defaults to `"new"` when omitted.
    #[serde(default = "default_side")]
    pub side: String,
    /// 1-based anchor line on `side`; 0 for a PR-level comment.
    pub line: u64,
    /// `info` / `suggestion` / `issue` / `blocker`; defaults to `suggestion` when
    /// omitted. Unknown values are mapped by the caller.
    #[serde(default = "default_severity")]
    pub severity: String,
    /// What kind of remark this is, e.g. "coding-standards".
    #[serde(default)]
    pub tag: Option<String>,
    /// Markdown body.
    pub body: String,
    /// Set when the comment is a reply within an existing thread.
    #[serde(default)]
    pub parent_id: Option<String>,
    /// Inclusive end line for a multi-line anchor.
    #[serde(default)]
    pub end_line: Option<u64>,
    /// Exact replacement code for the anchored lines (a committable
    /// suggested change, GitHub-style).
    #[serde(default)]
    pub suggestion: Option<String>,
}

fn default_side() -> String {
    "new".to_owned()
}

fn default_severity() -> String {
    "suggestion".to_owned()
}

#[derive(Debug, Deserialize)]
struct TypedLine {
    #[serde(rename = "type")]
    kind: String,
}

/// Classify one line of runner stdout.
pub fn classify_line(line: &str) -> RunEventKind {
    let trimmed = line.trim();
    if !trimmed.starts_with('{') {
        return RunEventKind::Raw;
    }
    match serde_json::from_str::<TypedLine>(trimmed) {
        Ok(t) if t.kind == "tandem_comment" => RunEventKind::Comment,
        Ok(t) if t.kind == "tandem_summary" => RunEventKind::Summary,
        Ok(_) => RunEventKind::Runner,
        Err(_) => RunEventKind::Raw,
    }
}

/// The payload an agent emits for the PR-level summary:
/// `{"type":"tandem_summary","body":"<markdown>"}`
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct AgentSummary {
    /// Markdown body of the summary.
    pub body: String,
}

/// Parse a `tandem_summary` line. `None` when the line isn't one.
pub fn parse_summary(line: &str) -> Option<AgentSummary> {
    let trimmed = line.trim();
    if classify_line(trimmed) != RunEventKind::Summary {
        return None;
    }
    let parsed: AgentSummary = serde_json::from_str(trimmed).ok()?;
    (!parsed.body.trim().is_empty()).then_some(parsed)
}

/// Parse an `tandem_comment` line. Returns `None` when the line is not a
/// valid comment payload.
pub fn parse_comment(line: &str) -> Option<AgentComment> {
    let trimmed = line.trim();
    if classify_line(trimmed) != RunEventKind::Comment {
        return None;
    }
    serde_json::from_str(trimmed).ok()
}

impl AgentComment {
    /// Canonical re-serialization (always includes the `type` tag).
    pub fn to_payload(&self) -> String {
        serde_json::json!({
            "type": "tandem_comment",
            "path": self.path,
            "side": self.side,
            "line": self.line,
            "severity": self.severity,
            "tag": self.tag,
            "body": self.body,
            "parent_id": self.parent_id,
            "end_line": self.end_line,
            "suggestion": self.suggestion,
        })
        .to_string()
    }

    /// Key for de-duplicating the same comment arriving via multiple
    /// channels (stdout line, embedded assistant text, comments file).
    pub fn dedupe_key(&self) -> String {
        format!(
            "{}|{}|{}|{}|{}",
            self.path,
            self.side,
            self.line,
            self.body,
            self.parent_id.as_deref().unwrap_or("")
        )
    }
}

/// Pull `tandem_comment` lines out of a structured runner envelope.
///
/// Headless claude wraps everything in stream-json envelopes, so when
/// the model "prints" a comment it actually lands inside an assistant
/// text block or the final `result` text — this digs them out.
pub fn extract_embedded_comments(payload: &str) -> Vec<AgentComment> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(payload) else {
        return Vec::new();
    };
    let mut texts: Vec<&str> = Vec::new();
    match value.get("type").and_then(|t| t.as_str()) {
        Some("assistant") => {
            let items = value.pointer("/message/content").and_then(|c| c.as_array());
            for item in items.into_iter().flatten() {
                if item.get("type").and_then(|t| t.as_str()) == Some("text") {
                    if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                        texts.push(text);
                    }
                }
            }
        }
        Some("result") => {
            if let Some(text) = value.get("result").and_then(|t| t.as_str()) {
                texts.push(text);
            }
        }
        // Grok Build streaming-json: model text arrives as top-level
        // `{"type":"text","text":"..."}` events.
        Some("text") => {
            if let Some(text) = value.get("text").and_then(|t| t.as_str()) {
                texts.push(text);
            }
        }
        _ => {}
    }
    texts
        .into_iter()
        .flat_map(str::lines)
        .filter_map(parse_comment)
        .collect()
}

/// Same dig as [`extract_embedded_comments`], for the single
/// `tandem_summary` line. Takes the last one if the agent emitted more
/// than one — that's its latest word on the PR.
pub fn extract_embedded_summary(payload: &str) -> Option<AgentSummary> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(payload) else {
        return None;
    };
    let mut texts: Vec<&str> = Vec::new();
    match value.get("type").and_then(|t| t.as_str()) {
        Some("assistant") => {
            let items = value.pointer("/message/content").and_then(|c| c.as_array());
            for item in items.into_iter().flatten() {
                if item.get("type").and_then(|t| t.as_str()) == Some("text") {
                    if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                        texts.push(text);
                    }
                }
            }
        }
        Some("result") => {
            if let Some(text) = value.get("result").and_then(|t| t.as_str()) {
                texts.push(text);
            }
        }
        Some("text") => {
            if let Some(text) = value.get("text").and_then(|t| t.as_str()) {
                texts.push(text);
            }
        }
        _ => {}
    }
    texts
        .into_iter()
        .flat_map(str::lines)
        .filter_map(parse_summary)
        .next_back()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn classifies_lines() {
        assert_eq!(classify_line("plain text"), RunEventKind::Raw);
        assert_eq!(classify_line("{not json"), RunEventKind::Raw);
        assert_eq!(
            classify_line(r#"{"type":"assistant","message":"hi"}"#),
            RunEventKind::Runner
        );
        assert_eq!(
            classify_line(r#"{"type":"tandem_comment","path":"a.rs","line":1,"body":"x"}"#),
            RunEventKind::Comment
        );
    }

    #[test]
    fn parses_comment_with_defaults() {
        let c = parse_comment(r#"{"type":"tandem_comment","path":"a.rs","line":7,"body":"nit"}"#)
            .unwrap();
        assert_eq!(c.path, "a.rs");
        assert_eq!(c.line, 7);
        assert_eq!(c.side, "new");
        assert_eq!(c.severity, "suggestion");
    }

    #[test]
    fn rejects_non_comment_json() {
        assert!(parse_comment(r#"{"type":"assistant"}"#).is_none());
        assert!(parse_comment(r#"{"type":"tandem_comment","path":"a.rs"}"#).is_none());
    }

    #[test]
    fn extracts_comments_from_assistant_text() {
        let comment = r#"{"type":"tandem_comment","path":"a.rs","line":3,"body":"off by one"}"#;
        let envelope = serde_json::json!({
            "type": "assistant",
            "message": { "content": [
                { "type": "text", "text": format!("Here you go:\n{comment}\ndone") },
                { "type": "tool_use", "name": "Read" }
            ]}
        })
        .to_string();
        let found = extract_embedded_comments(&envelope);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].path, "a.rs");
        assert_eq!(found[0].line, 3);
    }

    #[test]
    fn extracts_comments_from_result_text() {
        let comment = r#"{"type":"tandem_comment","path":"b.ts","line":9,"body":"nit"}"#;
        let envelope = serde_json::json!({
            "type": "result",
            "subtype": "success",
            "result": format!("summary\n{comment}")
        })
        .to_string();
        let found = extract_embedded_comments(&envelope);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].path, "b.ts");
    }

    #[test]
    fn parses_summary_and_rejects_empty_or_wrong_type() {
        let line =
            r#"{"type":"tandem_summary","body":"Solid refactor. See [a.rs:3](tandem://a.rs#L3)."}"#;
        assert_eq!(classify_line(line), RunEventKind::Summary);
        assert!(parse_summary(line)
            .unwrap()
            .body
            .contains("tandem://a.rs#L3"));
        // A blank summary is worse than none — it would blank out the
        // text a previous run wrote.
        assert!(parse_summary(r#"{"type":"tandem_summary","body":"   "}"#).is_none());
        assert!(
            parse_summary(r#"{"type":"tandem_comment","path":"a","line":1,"body":"x"}"#).is_none()
        );
    }

    #[test]
    fn extracts_summary_from_result_text() {
        let summary = r#"{"type":"tandem_summary","body":"overall: fine"}"#;
        let envelope = serde_json::json!({
            "type": "result",
            "subtype": "success",
            "result": format!("here you go\n{summary}")
        })
        .to_string();
        assert_eq!(
            extract_embedded_summary(&envelope).unwrap().body,
            "overall: fine"
        );
    }

    #[test]
    fn extracts_comments_from_grok_text_events() {
        let comment = r#"{"type":"tandem_comment","path":"c.go","line":4,"body":"leak"}"#;
        let envelope = serde_json::json!({
            "type": "text",
            "text": format!("analysis:\n{comment}")
        })
        .to_string();
        let found = extract_embedded_comments(&envelope);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].path, "c.go");
    }

    #[test]
    fn payload_roundtrips_and_dedupes() {
        let c = parse_comment(r#"{"type":"tandem_comment","path":"a.rs","line":7,"body":"x"}"#)
            .unwrap();
        let again = parse_comment(&c.to_payload()).unwrap();
        assert_eq!(c, again);
        assert_eq!(c.dedupe_key(), again.dedupe_key());
    }
}
