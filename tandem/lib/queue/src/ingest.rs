//! `beagle tandem ingest`: load an agent's review — the JSON lines described
//! in [`crate::context`] — into the store. All or nothing: every line is
//! checked first (shape, the anchor really is on that side of the diff,
//! the required history), and on any problem nothing is written and each
//! problem is reported with its line number, so the agent can fix the
//! file and run it again.

use std::collections::HashSet;

use serde::Deserialize;
use tandem_core::diff::{FileDiff, LineKind};
use tandem_core::review::{CommentSeverity, DiffSide};

use crate::verdict::Verdict;

/// One review comment, as the agent wrote it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentLine {
    pub path: String,
    #[serde(default = "new_side")]
    pub side: DiffSide,
    pub line: u64,
    #[serde(default)]
    pub end_line: Option<u64>,
    pub severity: CommentSeverity,
    #[serde(default)]
    pub tag: Option<String>,
    pub body: String,
    #[serde(default)]
    pub suggestion: Option<String>,
    pub context: String,
    #[serde(default)]
    pub duplicate_of: Option<String>,
}

fn new_side() -> DiffSide {
    DiffSide::New
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum Line {
    #[serde(rename = "tandem_comment")]
    Comment(AgentLine),
    #[serde(rename = "tandem_summary")]
    Summary { body: String },
    #[serde(rename = "tandem_verdict")]
    Verdict { verdict: String, reason: String },
}

/// A review that passed every check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Review {
    pub verdict: Verdict,
    pub reason: String,
    pub summary: Option<String>,
    pub comments: Vec<AgentLine>,
}

/// Parses and checks `text` against the PR's diff. `summary_exists`
/// says whether an earlier review already wrote the summary.
pub fn parse(text: &str, diff: &[FileDiff], summary_exists: bool) -> Result<Review, Vec<String>> {
    let mut errors = Vec::new();
    let mut verdicts = Vec::new();
    let mut summaries = Vec::new();
    let mut comments = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let raw = raw.trim();
        if raw.is_empty() {
            continue;
        }
        let at = i + 1;
        match serde_json::from_str::<Line>(raw) {
            Err(e) => errors.push(format!("line {at}: not a valid review line: {e}")),
            Ok(Line::Verdict { verdict, reason }) => match Verdict::from_key(&verdict) {
                Some(v) if !reason.trim().is_empty() => verdicts.push((v, reason)),
                Some(_) => errors.push(format!("line {at}: the verdict needs a reason")),
                None => errors.push(format!(
                    "line {at}: unknown verdict {verdict:?}; use one of {}",
                    keys()
                )),
            },
            Ok(Line::Summary { body }) if body.trim().is_empty() => {
                errors.push(format!("line {at}: the summary is empty"));
            }
            Ok(Line::Summary { body }) => summaries.push(body),
            Ok(Line::Comment(c)) => {
                errors.extend(
                    check_comment(&c, diff)
                        .into_iter()
                        .map(|e| format!("line {at}: {e}")),
                );
                comments.push(c);
            }
        }
    }
    if verdicts.len() != 1 {
        errors.push(format!(
            "expected exactly one tandem_verdict line, found {}",
            verdicts.len()
        ));
    }
    match (summary_exists, summaries.len()) {
        (true, 0) | (false, 1) => {}
        (true, _) => errors.push(
            "a summary already exists for this PR: remove the tandem_summary line".to_owned(),
        ),
        (false, n) => errors.push(format!(
            "expected exactly one tandem_summary line, found {n}"
        )),
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    let Some((verdict, reason)) = verdicts.pop() else {
        return Err(vec!["no verdict".to_owned()]);
    };
    Ok(Review {
        verdict,
        reason,
        summary: summaries.pop(),
        comments,
    })
}

fn keys() -> String {
    Verdict::ALL.map(Verdict::key).join(", ")
}

fn check_comment(c: &AgentLine, diff: &[FileDiff]) -> Vec<String> {
    let mut errors = Vec::new();
    if c.body.trim().is_empty() {
        errors.push("the body is empty".to_owned());
    }
    if c.context.trim().is_empty() {
        errors.push("the context (the local history reply) is required".to_owned());
    }
    let duplicate = c.tag.as_deref() == Some("duplicate");
    let has_ref = c
        .duplicate_of
        .as_deref()
        .is_some_and(|r| !r.trim().is_empty());
    if duplicate && !has_ref {
        errors.push("a duplicate needs duplicate_of: who said it and where".to_owned());
    }
    if c.path.is_empty() {
        if c.line != 0 {
            errors.push("a PR-level comment (empty path) must use line 0".to_owned());
        }
        return errors;
    }
    let Some(file) = diff
        .iter()
        .find(|f| f.new_path == c.path || f.old_path == c.path)
    else {
        errors.push(format!("{} is not a file in this diff", c.path));
        return errors;
    };
    let lines = anchorable(file, c.side);
    let side = match c.side {
        DiffSide::New => "new",
        DiffSide::Old => "old",
    };
    let end = c.end_line.unwrap_or(c.line);
    if end < c.line {
        errors.push(format!("end_line {end} is before line {}", c.line));
    }
    for n in [c.line, end] {
        if !lines.contains(&n) {
            errors.push(format!(
                "{}:{n} is not on the {side} side of the diff (comment on a changed or context line)",
                c.path
            ));
            break;
        }
    }
    errors
}

/// The line numbers a comment can anchor to on `side` of `file`.
fn anchorable(file: &FileDiff, side: DiffSide) -> HashSet<u64> {
    file.hunks
        .iter()
        .flat_map(|h| &h.lines)
        .filter_map(|l| match (side, l.kind) {
            (DiffSide::New, LineKind::Added | LineKind::Context) => l.new_line,
            (DiffSide::Old, LineKind::Removed | LineKind::Context) => l.old_line,
            _ => None,
        })
        .collect()
}

#[cfg(test)]
#[path = "ingest_tests.rs"]
mod tests;
