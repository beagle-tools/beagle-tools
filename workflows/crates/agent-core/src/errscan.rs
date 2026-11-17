//! What broke since a point in time — the deterministic half of the
//! pipeline resolver.
//!
//! [`scan`] reads the store, never the filesystem's logs: a run's `ERROR`
//! lines, final `result` and state survive log pruning for a week (see
//! `prune.rs`), so a 48h window is fully covered. Two things count:
//!
//! * a run that ended **failed** or **timed out** — one occurrence, with the
//!   best reason found (its `ERROR` lines, else its runner's error result,
//!   else the exit code);
//! * `ERROR` lines in a run that otherwise ended fine — one occurrence per
//!   run, its distinct lines together. A gate that logs "cannot
//!   authenticate" and carries on is still broken.
//!
//! Killed runs are not errors: someone chose to stop them.
//!
//! Occurrences are deduplicated before anything reads them: the same step
//! failing the same way is one [`ErrorGroup`] with a count, keyed on the
//! message with run ids, timestamps, hashes and numbers masked out.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::state::{RunFilter, Store};
use crate::types::{Run, RunState};
use crate::CoreError;

/// How many example runs a group lists, newest first.
const SAMPLE_RUNS: usize = 5;
/// How many `ERROR` lines make a failed run's reason.
const REASON_LINES: usize = 3;
/// Longest message kept, in characters.
const MESSAGE_CHARS: usize = 600;

/// What kind of breakage a group is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    /// The run ended `failed`.
    Failed,
    /// The run ended `timedout`.
    TimedOut,
    /// An `ERROR` line in a run that did not fail.
    Logged,
}

/// One run named in a group.
#[derive(Debug, Clone, Serialize)]
pub struct RunRef {
    /// The run id.
    pub id: String,
    /// How it ended (`failed`, `timedout`, `succeeded`, …).
    pub state: String,
    /// When it started, if it did.
    pub started_at: Option<DateTime<Utc>>,
    /// When it finished.
    pub finished_at: Option<DateTime<Utc>>,
    /// Its params, as the JSON stored on the run.
    pub params: Option<String>,
    /// The log file, while it still exists (4h by default).
    pub log_path: Option<PathBuf>,
    /// The data dir (`$AGENTD_DATA_DIR`), while it still exists (a week).
    pub data_dir: Option<PathBuf>,
}

/// One kind of breakage in one step, however many times it happened.
#[derive(Debug, Clone, Serialize)]
pub struct ErrorGroup {
    /// `project/agent` of the chain's root — the job.
    pub job: String,
    /// `project/agent` of the step that broke.
    pub step: String,
    /// Whether the run ended broken, or only logged errors.
    pub kind: ErrorKind,
    /// The newest occurrence's message, verbatim.
    pub message: String,
    /// The masked message the group is keyed on.
    pub signature: String,
    /// How many runs hit it in the window.
    pub count: usize,
    /// The oldest occurrence in the window.
    pub first_seen: DateTime<Utc>,
    /// The newest occurrence in the window.
    pub last_seen: DateTime<Utc>,
    /// The step's manifest, when the agent is still declared.
    pub manifest: Option<PathBuf>,
    /// The job's `job-purpose.md`, when declared.
    pub purpose: Option<PathBuf>,
    /// Every run in the group.
    pub runs: Vec<RunRef>,
}

/// Everything that broke in runs that finished in `(since, now]`, grouped
/// and newest first. `data_root` is where run data dirs live.
pub fn scan(
    store: &Store,
    since: DateTime<Utc>,
    now: DateTime<Utc>,
    data_root: &Path,
) -> Result<Vec<ErrorGroup>, CoreError> {
    let runs = store.list_runs(&RunFilter::default())?;
    let by_id: HashMap<&str, &Run> = runs.iter().map(|r| (r.id.as_str(), r)).collect();
    let specs: HashMap<String, crate::types::AgentSpec> = store
        .list_agents()?
        .into_iter()
        .map(|s| (s.qualified_name(), s))
        .collect();

    let mut groups: BTreeMap<(String, ErrorKind, String), ErrorGroup> = BTreeMap::new();
    for run in &runs {
        let Some(finished) = run.finished_at else {
            continue;
        };
        if finished <= since || finished > now {
            continue;
        }
        let (errors, result) = evidence(store, &run.id)?;
        let occurrences: Vec<(ErrorKind, String)> = match run.state {
            RunState::Failed | RunState::TimedOut => {
                let kind = if run.state == RunState::Failed {
                    ErrorKind::Failed
                } else {
                    ErrorKind::TimedOut
                };
                vec![(kind, reason(run, &errors, result.as_deref()))]
            }
            // One occurrence per run, all its ERROR lines together: a step
            // that logs a multi-line finding is one problem, not five.
            RunState::Succeeded if !errors.is_empty() => {
                let mut lines: Vec<String> = Vec::new();
                for line in errors {
                    if !line.trim_end_matches(':').trim().is_empty() && !lines.contains(&line) {
                        lines.push(line);
                    }
                }
                vec![(ErrorKind::Logged, clip(&lines.join(" | ")))]
            }
            _ => Vec::new(),
        };
        for (kind, message) in occurrences {
            let step = format!("{}/{}", run.project, run.agent);
            let signature = signature(&message);
            let job = root_of(run, &by_id);
            let entry = groups
                .entry((step.clone(), kind, signature.clone()))
                .or_insert_with(|| ErrorGroup {
                    job: job.clone(),
                    purpose: specs.get(&job).and_then(|s| s.purpose.clone()),
                    manifest: specs.get(&step).map(|s| s.source.clone()),
                    step,
                    kind,
                    message: message.clone(),
                    signature,
                    count: 0,
                    first_seen: finished,
                    last_seen: finished,
                    runs: Vec::new(),
                });
            entry.count += 1;
            if finished < entry.first_seen {
                entry.first_seen = finished;
            }
            if finished >= entry.last_seen {
                entry.last_seen = finished;
                entry.message = message;
                entry.job = job;
            }
            if !entry.runs.iter().any(|r| r.id == run.id) {
                entry.runs.push(run_ref(run, data_root));
            }
        }
    }

    let mut out: Vec<ErrorGroup> = groups.into_values().collect();
    for group in &mut out {
        group.runs.sort_by_key(|r| std::cmp::Reverse(r.finished_at));
        group.runs.truncate(SAMPLE_RUNS);
    }
    out.sort_by_key(|g| std::cmp::Reverse(g.last_seen));
    Ok(out)
}

/// The run's `ERROR` lines (prefix stripped) and its runner's final result
/// text when that result is an error.
fn evidence(store: &Store, run_id: &str) -> Result<(Vec<String>, Option<String>), CoreError> {
    let mut errors = Vec::new();
    let mut result = None;
    let mut after = 0;
    loop {
        let batch = store.events_after(run_id, after, 500)?;
        let Some(last) = batch.last() else {
            break;
        };
        after = last.id;
        for event in &batch {
            let Some(payload) = event.payload.as_deref() else {
                continue;
            };
            match event.kind.as_str() {
                "raw" => {
                    if let Some(line) = payload.strip_prefix("ERROR ") {
                        errors.push(clip(line.trim()));
                    }
                }
                "result" => result = result_error(payload).or(result),
                _ => {}
            }
        }
    }
    Ok((errors, result))
}

/// A runner `result` event's text, when it reports an error.
fn result_error(payload: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(payload).ok()?;
    if value.get("is_error").and_then(serde_json::Value::as_bool) != Some(true) {
        return None;
    }
    let text = value
        .get("result")
        .and_then(serde_json::Value::as_str)
        .or_else(|| value.get("subtype").and_then(serde_json::Value::as_str))
        .unwrap_or("runner reported an error");
    Some(clip(text.trim()))
}

/// Why a failed or timed-out run ended, in one message.
fn reason(run: &Run, errors: &[String], result: Option<&str>) -> String {
    if !errors.is_empty() {
        let start = errors.len().saturating_sub(REASON_LINES);
        return clip(&errors[start..].join(" | "));
    }
    if let Some(text) = result {
        return text.to_owned();
    }
    match (run.state, run.exit_code) {
        (RunState::TimedOut, _) => "timed out (no ERROR line logged)".to_owned(),
        (_, Some(code)) => format!("exited {code} (no ERROR line logged)"),
        _ => "failed (no ERROR line logged)".to_owned(),
    }
}

/// `project/agent` of the chain's root run.
fn root_of(run: &Run, by_id: &HashMap<&str, &Run>) -> String {
    let mut cur = run;
    for _ in 0..64 {
        match cur.parent_run_id.as_deref().and_then(|p| by_id.get(p)) {
            Some(parent) => cur = parent,
            None => break,
        }
    }
    format!("{}/{}", cur.project, cur.agent)
}

fn run_ref(run: &Run, data_root: &Path) -> RunRef {
    let data_dir = data_root.join(&run.id);
    RunRef {
        id: run.id.clone(),
        state: run.state.as_str().to_owned(),
        started_at: run.started_at,
        finished_at: run.finished_at,
        params: run.params.clone(),
        log_path: run.log_path.clone().filter(|p| p.is_file()),
        data_dir: data_dir.is_dir().then_some(data_dir),
    }
}

fn clip(text: &str) -> String {
    if text.chars().count() <= MESSAGE_CHARS {
        return text.to_owned();
    }
    let mut out: String = text.chars().take(MESSAGE_CHARS).collect();
    out.push('…');
    out
}

/// `message` with what differs between occurrences of the same error
/// masked: digits (counts, durations, PR numbers, timestamps) become `#`,
/// and any word holding a digit — run ids, hashes, versions — becomes
/// `<id>`. Whitespace collapses. Two failures with the same signature are
/// the same failure.
pub fn signature(message: &str) -> String {
    let mut words = Vec::new();
    for word in message.split_whitespace() {
        let digits = word.chars().filter(char::is_ascii_digit).count();
        let letters = word.chars().filter(char::is_ascii_alphabetic).count();
        if digits > 0 && letters > 0 {
            // Keep the punctuation around it so "(run-…)" still reads.
            let lead: String = word.chars().take_while(|c| !c.is_alphanumeric()).collect();
            let tail: String = word
                .chars()
                .rev()
                .take_while(|c| !c.is_alphanumeric())
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            words.push(format!("{lead}<id>{tail}"));
        } else {
            words.push(
                word.chars()
                    .map(|c| if c.is_ascii_digit() { '#' } else { c })
                    .collect(),
            );
        }
    }
    let mut out = words.join(" ");
    // A run of masks is one mask: "##:##:##" and "#.#" read the same.
    while out.contains("##") {
        out = out.replace("##", "#");
    }
    out
}

#[cfg(test)]
// Tests should fail loudly on a broken fixture; unwrapping is the clearest way.
#[allow(clippy::unwrap_used)]
#[path = "tests/errscan.rs"]
mod tests;
