//! What a run cost, pulled out of the runner's own final event.
//!
//! Claude's `result` line reports `total_cost_usd` directly — it is the
//! runner's own accounting, not a price table we maintain, so it stays right
//! when pricing changes. Codex reports tokens on `turn.completed` and no
//! price; rather than multiply by hardcoded rates that would go stale
//! silently, cost stays `None` for codex and only the usage detail is kept.
//! A blank cost is honest; a confidently wrong one is not.

use serde_json::Value;

/// Accounting for one run, as its runner reported it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RunUsage {
    /// `None` for runners that do not report a price (codex).
    pub cost_usd: Option<f64>,
    pub num_turns: Option<i64>,
    /// The raw usage object, kept whole so token/cache detail survives
    /// without a column per field.
    pub usage_json: Option<String>,
}

impl RunUsage {
    fn is_empty(&self) -> bool {
        self.cost_usd.is_none() && self.num_turns.is_none() && self.usage_json.is_none()
    }
}

/// Parse a runner's terminal accounting line, if this is one.
///
/// Returns `None` for every other line, so the caller can hand it every log
/// line during ingestion without pre-filtering.
pub fn parse_line(line: &str) -> Option<RunUsage> {
    let json: Value = serde_json::from_str(line).ok()?;
    let kind = json.get("type").and_then(Value::as_str)?;
    // Claude: one `result` per run. Codex: one `turn.completed` per turn, so
    // the last one seen wins — which is what we want.
    if kind != "result" && kind != "turn.completed" {
        return None;
    }
    let usage = RunUsage {
        cost_usd: json.get("total_cost_usd").and_then(Value::as_f64),
        num_turns: json.get("num_turns").and_then(Value::as_i64),
        usage_json: json.get("usage").map(ToString::to_string),
    };
    if usage.is_empty() {
        return None;
    }
    Some(usage)
}

#[cfg(test)]
// Tests should fail loudly on a broken fixture; unwrapping is the clearest way.
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn claude_result_carries_cost_and_turns() {
        let line = r#"{"type":"result","subtype":"success","total_cost_usd":1.0436137,
            "num_turns":57,"usage":{"input_tokens":64,"output_tokens":25237,
            "cache_read_input_tokens":2517651}}"#;
        let u = parse_line(line).unwrap();
        assert!((u.cost_usd.unwrap() - 1.043_613_7).abs() < 1e-9);
        assert_eq!(u.num_turns, Some(57));
        // Token detail survives whole rather than needing a column each.
        assert!(u.usage_json.unwrap().contains("2517651"));
    }

    #[test]
    fn codex_turn_has_usage_but_no_price() {
        let line = r#"{"type":"turn.completed","usage":{"input_tokens":10,"output_tokens":20}}"#;
        let u = parse_line(line).unwrap();
        assert_eq!(
            u.cost_usd, None,
            "codex reports no price; do not invent one"
        );
        assert!(u.usage_json.is_some());
    }

    #[test]
    fn other_lines_are_ignored() {
        assert!(parse_line(r#"{"type":"assistant","message":{}}"#).is_none());
        assert!(parse_line("not json").is_none());
        assert!(parse_line(r#"{"no":"type"}"#).is_none());
        // A result with nothing useful in it is not worth a write.
        assert!(parse_line(r#"{"type":"result","subtype":"success"}"#).is_none());
    }
}
