//! Fitting one runner output line into the event store.
//!
//! Events are capped at 4 KB to keep the store small. The cap used to be
//! a byte cut — which, on a stream-json line, cuts the JSON in half. A
//! tool result carrying a whole file is far past 4 KB, so exactly the
//! events worth reading were stored as unparseable fragments, and every
//! log view fell back to printing the raw text of them.
//!
//! So a JSON line is shrunk *structurally*: long string values inside it
//! are clipped, and the result is re-serialised. It stays valid, keeps
//! its `type` and every short field, and loses only the middle of things
//! nobody reads in full in a log view anyway. Plain-text lines — script
//! output — are still cut by bytes; there is nothing to break.

use beagle_utils::text::truncate_bytes;
use serde_json::Value;

pub(crate) const MAX_PAYLOAD_BYTES: usize = 4096;

/// Successively harsher clips, tried until the line fits. Most lines fit
/// at the first: one big tool result is the usual offender.
const STRING_CAPS: [usize; 4] = [800, 300, 100, 40];

pub(crate) fn fit(line: &str) -> String {
    if line.len() <= MAX_PAYLOAD_BYTES {
        return line.to_owned();
    }
    let Ok(mut json) = serde_json::from_str::<Value>(line) else {
        return truncate_bytes(line, MAX_PAYLOAD_BYTES).to_owned();
    };
    // Claude repeats a tool's output at the top level of a `user` event,
    // beside the tool_result block that already carries it. It is the
    // single largest thing in those events and no view reads it.
    if let Some(object) = json.as_object_mut() {
        object.remove("tool_use_result");
    }
    for cap in STRING_CAPS {
        clip_strings(&mut json, cap);
        if let Ok(text) = serde_json::to_string(&json) {
            if text.len() <= MAX_PAYLOAD_BYTES {
                return text;
            }
        }
    }
    // Thousands of short fields will not clip under the cap. Keep the
    // event's identity, valid, and say so — never a broken fragment.
    let kind = json.get("type").cloned().unwrap_or(Value::Null);
    serde_json::json!({ "type": kind, "clipped": true }).to_string()
}

fn clip_strings(value: &mut Value, cap: usize) {
    match value {
        Value::String(text) => {
            let len = text.chars().count();
            if len > cap {
                let kept: String = text.chars().take(cap).collect();
                *text = format!("{kept}… [+{} chars]", len - cap);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|v| clip_strings(v, cap)),
        Value::Object(fields) => fields.values_mut().for_each(|v| clip_strings(v, cap)),
        _ => {}
    }
}

#[cfg(test)]
// Tests should fail loudly on a broken fixture; unwrapping is the clearest way.
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn a_huge_tool_result_stays_valid_json_under_the_cap() {
        // The shape that used to be stored as a broken fragment: a user
        // event whose tool result is an entire file.
        let file = "line of a big file\n".repeat(2000);
        let line = serde_json::json!({
            "type": "user",
            "message": { "role": "user", "content": [
                { "type": "tool_result", "tool_use_id": "t1", "content": file }
            ]},
            "tool_use_result": { "type": "text", "file": { "content": file } }
        })
        .to_string();
        assert!(line.len() > 40_000);

        let fitted = fit(&line);
        assert!(fitted.len() <= MAX_PAYLOAD_BYTES, "{} bytes", fitted.len());
        let json: Value = serde_json::from_str(&fitted).expect("must still be JSON");
        assert_eq!(json["type"], "user");
        assert!(json.get("tool_use_result").is_none());
        let kept = json
            .pointer("/message/content/0/content")
            .unwrap()
            .as_str()
            .unwrap();
        assert!(kept.starts_with("line of a big file"));
        assert!(kept.contains("chars]"), "the clip says how much it dropped");
    }

    #[test]
    fn a_line_that_already_fits_is_untouched() {
        let line = r#"{"type":"assistant","message":{"content":[]}}"#;
        assert_eq!(fit(line), line);
    }

    #[test]
    fn plain_text_is_cut_by_bytes() {
        let line = "x".repeat(MAX_PAYLOAD_BYTES * 2);
        assert_eq!(fit(&line).len(), MAX_PAYLOAD_BYTES);
    }

    #[test]
    fn many_small_fields_degrade_to_a_valid_marker() {
        let fields: serde_json::Map<String, Value> = (0..3000)
            .map(|i| (format!("k{i}"), Value::from(i)))
            .collect();
        let mut json = Value::Object(fields);
        json["type"] = Value::from("system");
        let fitted = fit(&json.to_string());
        let parsed: Value = serde_json::from_str(&fitted).unwrap();
        assert_eq!(parsed["type"], "system");
        assert_eq!(parsed["clipped"], true);
    }
}
