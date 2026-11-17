//! Pulling URLs back out of the files agents write.
//!
//! A stage reports its PR by writing the URL into `result.md` or its
//! write-up, in prose, for a human. Three things then need to find it
//! again: retention (keep the write-up while the PR is open), agenttop
//! and Agentdesk (offer the link), and they must agree — a PR that the
//! sweeper can see but the UI cannot is a write-up that vanishes with no
//! way to have reached it.
//!
//! This lived in two places and had already drifted: one trimmed a
//! trailing `>` and the other did not, so `<https://…/pull/9>` resolved
//! to a different URL depending on who asked.

use std::path::Path;

/// Punctuation that is part of the prose, not the URL. Markdown wraps
/// links in `<>`, `[]`, backticks and parens, and a sentence ends with
/// `.` or `,` — none of which belong in the href.
const TRAILING: [char; 8] = [')', ',', '.', '`', '>', ']', ';', '!'];

/// The first `https://` URL containing `needle`, trimmed of trailing
/// prose punctuation.
///
/// The URL is found WITHIN the word rather than at the start of it,
/// because `[#1333](https://…/pull/1333)` is how a model asked to "link
/// the PR" most often writes one, and splitting on whitespace there
/// yields `PR](https://…`. Both earlier copies of this required the
/// word to begin with `https://` and so missed every markdown inline
/// link — which, for retention, meant deleting a write-up whose PR was
/// still open.
pub fn first_url(body: &str, needle: &str) -> Option<String> {
    body.split_whitespace()
        .filter_map(|word| word.find("https://").map(|at| &word[at..]))
        .find(|url| url.contains(needle))
        .map(|url| url.trim_end_matches(TRAILING).to_owned())
}

/// [`first_url`] against a file. A missing or unreadable file is `None`:
/// every caller is asking "is there a link here", and an unreadable file
/// answers that question the same way an absent one does.
pub fn first_url_in_file(path: &Path, needle: &str) -> Option<String> {
    first_url(&std::fs::read_to_string(path).ok()?, needle)
}

/// The first GitHub pull-request URL in a file.
pub fn pr_url_in_file(path: &Path) -> Option<String> {
    let body = std::fs::read_to_string(path).ok()?;
    first_url(&body, "/pull/").filter(|u| u.starts_with("https://github.com/"))
}

/// The PR a run OPENED — not one it mentions.
///
/// The run's own events decide: the `gh pr create` call and, in its
/// result, the URL gh printed, paired by tool-use id. When the events
/// exist and hold no such call, the run opened no PR, full stop — so a
/// stage that merely writes the chain's PR into its notes (docs, report)
/// is never taken for the one that opened it. Only when the events are
/// gone does `result.md` stand in, since the implement stage records its
/// PR there. Works mid-run too: the call is in the events the moment it
/// happens, long before result.md is written.
pub fn run_pr(store: &crate::state::Store, run_id: &str) -> Option<String> {
    if let Ok(events) = store.events_after(run_id, 0, 5000) {
        if !events.is_empty() {
            return pr_created_in(&events);
        }
    }
    let data = crate::paths::data_dir().ok()?;
    pr_url_in_file(&data.join(run_id).join("result.md"))
}

/// The URL `gh pr create` printed, found by pairing the tool call with its
/// result. Pure, so it is tested without a store.
pub fn pr_created_in(events: &[crate::types::Event]) -> Option<String> {
    use serde_json::Value;
    let blocks = |payload: &str| -> Vec<Value> {
        serde_json::from_str::<Value>(payload)
            .ok()
            .and_then(|v| {
                v.pointer("/message/content")
                    .and_then(Value::as_array)
                    .cloned()
            })
            .unwrap_or_default()
    };
    let mut creates: Vec<String> = Vec::new();
    for event in events {
        let Some(payload) = event.payload.as_deref() else {
            continue;
        };
        for block in blocks(payload) {
            match block.get("type").and_then(Value::as_str) {
                Some("tool_use") => {
                    let command = block
                        .pointer("/input/command")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    if command.contains("gh pr create") {
                        if let Some(id) = block.get("id").and_then(Value::as_str) {
                            creates.push(id.to_owned());
                        }
                    }
                }
                Some("tool_result") => {
                    let id = block
                        .get("tool_use_id")
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    if !creates.iter().any(|c| c == id) {
                        continue;
                    }
                    let text = match block.get("content") {
                        Some(Value::String(s)) => s.clone(),
                        Some(Value::Array(parts)) => parts
                            .iter()
                            .filter_map(|p| p.get("text").and_then(Value::as_str))
                            .collect::<Vec<_>>()
                            .join("\n"),
                        _ => String::new(),
                    };
                    if let Some(url) = first_url(&text, "/pull/") {
                        return Some(url);
                    }
                }
                _ => {}
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const PR: &str = "https://github.com/acme/widgets/pull/1333";

    #[test]
    fn trailing_prose_punctuation_is_not_part_of_the_url() {
        for wrapped in [
            format!("- **PR:** {PR} (draft)\n"),
            format!("see {PR}.\n"),
            format!("opened {PR}, then tested\n"),
            format!("`{PR}`\n"),
            format!("<{PR}>\n"),
            format!("[the PR]({PR})\n"),
        ] {
            assert_eq!(
                first_url(&wrapped, "/pull/").as_deref(),
                Some(PR),
                "{wrapped:?}"
            );
        }
    }

    #[test]
    fn the_first_match_wins() {
        let body = format!("{PR}\nand later https://github.com/x/y/pull/2\n");
        assert_eq!(first_url(&body, "/pull/").as_deref(), Some(PR));
    }

    #[test]
    fn a_url_that_does_not_match_the_needle_is_ignored() {
        let body = "https://github.com/acme/widgets/issues/4\n";
        assert_eq!(first_url(body, "/pull/"), None);
    }

    #[test]
    fn a_markdown_inline_link_is_found() {
        // The shape a model actually writes when told to "link the PR".
        let body = format!("**Draft PR:** [#1333]({PR})\n");
        assert_eq!(first_url(&body, "/pull/").as_deref(), Some(PR));
    }

    #[test]
    fn http_is_not_matched() {
        // Only https. A plain-http github URL is a typo, not a PR link.
        assert_eq!(first_url("http://github.com/a/b/pull/1", "/pull/"), None);
    }

    #[test]
    fn no_url_is_none_rather_than_empty() {
        assert_eq!(first_url("no links here at all\n", "/pull/"), None);
    }

    fn event(payload: &str) -> crate::types::Event {
        crate::types::Event {
            id: 1,
            run_id: "r".to_owned(),
            ts: chrono::Utc::now(),
            kind: "assistant".to_owned(),
            payload: Some(payload.to_owned()),
        }
    }

    #[test]
    fn the_pr_a_run_opened_is_found_from_its_own_gh_call() {
        let events = vec![
            event(
                r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"t9","name":"Bash","input":{"command":"gh pr create --draft --title x"}}]}}"#,
            ),
            event(
                r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t9","content":"https://github.com/acme/widgets/pull/764\n"}]}}"#,
            ),
        ];
        assert_eq!(
            pr_created_in(&events).as_deref(),
            Some("https://github.com/acme/widgets/pull/764")
        );
    }

    #[test]
    fn a_pr_the_run_only_read_about_is_not_one_it_opened() {
        // `gh pr view` of someone else's PR prints a PR URL too.
        let events = vec![
            event(
                r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"t1","name":"Bash","input":{"command":"gh pr view 12 --repo a/b"}}]}}"#,
            ),
            event(
                r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t1","content":"https://github.com/a/b/pull/12"}]}}"#,
            ),
        ];
        assert_eq!(pr_created_in(&events), None);
    }
}
