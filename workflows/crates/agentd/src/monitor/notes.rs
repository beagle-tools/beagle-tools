//! The notes a finished run leaves in its log: its own announcement and
//! the repos it wrote to. Split from `monitor.rs`.

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use agent_core::paths;
use agent_core::state::Store;
use agent_core::types::Run;
use beagle_utils::process::output_with_timeout;
use tracing::info;

use super::GIT_TIMEOUT;

/// Persist a run's self-authored announcement (`$AGENTD_DATA_DIR/notify.txt`,
/// e.g. "auth failed — run the cloud login") as a `note` event, so the reason
/// shows in agenttop's activity column and log view, not just the banner.
pub(super) fn record_announcement(store: &Store, run: &Run) {
    let Ok(dir) = paths::data_dir() else { return };
    let Ok(text) = std::fs::read_to_string(dir.join(&run.id).join("notify.txt")) else {
        return;
    };
    let trimmed = text.trim();
    if !trimmed.is_empty() {
        let _ = store.insert_event(&run.id, "note", Some(trimmed));
    }
}

/// Records which repos a model run wrote to — commits, pushes, worktree
/// changes, PR and API writes — as one `note`, so every run's log names the
/// repos it touched (see `agent_core::repotrace`). Scripts log their own.
pub(super) fn record_repos(store: &Store, run: &Run) {
    use agent_core::repotrace::{self, Target};
    let Ok(events) = store.events_after(&run.id, 0, 20_000) else {
        return;
    };
    let workdir = run.workdir.clone().unwrap_or_default();
    let mut names: HashMap<std::path::PathBuf, String> = HashMap::new();
    let mut current: HashMap<std::path::PathBuf, Option<String>> = HashMap::new();
    let writes: Vec<(String, Option<String>, String)> = repotrace::bash_commands(&events)
        .iter()
        .flat_map(|c| repotrace::writes_in(c, &workdir))
        .map(|w| match w.target {
            Target::GitHub(name) => (name, w.branch, w.verb),
            Target::Local(path) => {
                let repo = names
                    .entry(path.clone())
                    .or_insert_with(|| local_repo_name(&path))
                    .clone();
                // The command named no branch: the checkout's branch now.
                let branch = w.branch.or_else(|| {
                    current
                        .entry(path.clone())
                        .or_insert_with(|| current_branch(&path))
                        .clone()
                });
                (repo, branch, w.verb)
            }
        })
        .collect();
    let protected = agent_core::guard::protected_orgs();
    if let Some(line) = repotrace::summary(&writes, &protected) {
        if line.contains("PUSHED TO MAIN") {
            tracing::error!(run = %run.id, "{line}");
        }
        info!(run = %run.id, "{line}");
        let _ = store.insert_event(&run.id, "note", Some(&line));
    }
}

/// A checkout's current branch; `None` when detached or gone.
fn current_branch(path: &Path) -> Option<String> {
    output_with_timeout(
        Command::new("git")
            .arg("-C")
            .arg(path)
            .args(["symbolic-ref", "--short", "-q", "HEAD"]),
        GIT_TIMEOUT,
    )
    .ok()
    .filter(|o| o.status.success())
    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
    .filter(|s| !s.is_empty())
}

/// `owner/name` of a checkout from its origin remote; the path itself when
/// it has none (or is gone — a removed worktree): its folder still says
/// which repo it was.
fn local_repo_name(path: &Path) -> String {
    output_with_timeout(
        Command::new("git")
            .arg("-C")
            .arg(path)
            .args(["remote", "get-url", "origin"]),
        GIT_TIMEOUT,
    )
    .ok()
    .filter(|o| o.status.success())
    .map(|o| {
        String::from_utf8_lossy(&o.stdout)
            .trim()
            .trim_start_matches("git@github.com:")
            .trim_start_matches("https://github.com/")
            .trim_end_matches(".git")
            .to_owned()
    })
    .filter(|s| !s.is_empty())
    .unwrap_or_else(|| path.display().to_string())
}
