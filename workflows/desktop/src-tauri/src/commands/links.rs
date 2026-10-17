//! The things a run points at: its write-up, its chain's write-up, the PR
//! it opened, the ticket it came from.
//!
//! This mirrors agenttop's `m` overlay deliberately. Both read the same
//! run row and the same files, so the two views cannot disagree about what
//! a run produced — which they would if the desktop app re-derived any of
//! it from a different source.

use std::path::PathBuf;

use agent_core::types::Run;
use tauri::State;

use crate::error::{lock_failed, AppError};
use crate::state::AppState;

#[derive(Debug, serde::Serialize)]
pub struct Link {
    pub label: String,
    /// `markdown` renders in-app; `url` opens in the browser; `file` is a
    /// path shown for copying.
    pub kind: String,
    pub target: String,
}

/// The chain write-up is listed FIRST when it exists. Standing on a stage,
/// the document worth reading is the one covering the whole ticket — the
/// per-stage note is the narrower thing.
#[tauri::command]
pub fn run_links(state: State<'_, AppState>, id: String) -> Result<Vec<Link>, AppError> {
    let store = state.store.lock().map_err(|_| lock_failed("store"))?;
    let run = store
        .get_run(&id)
        .map_err(|e| AppError::new(format!("could not read run {id}: {e}")))?
        .ok_or_else(|| AppError::new(format!("run {id} not found")))?;

    // Walk to the chain root, bounded — a cycle means a corrupt store, and
    // hanging the window is a worse way to find that out than stopping.
    let mut root = run.clone();
    for _ in 0..32 {
        let Some(parent) = root.parent_run_id.clone() else {
            break;
        };
        match store.get_run(&parent) {
            Ok(Some(next)) => root = next,
            _ => break,
        }
    }

    // Every run in this chain, for the PR hunt below.
    let all = store
        .list_runs(&agent_core::state::RunFilter {
            project: None,
            agent: None,
            states: None,
            limit: Some(200),
        })
        .unwrap_or_default();
    let lineage = agent_core::lineage::Lineage::build(&all);
    let chain_ids: Vec<String> = lineage
        .rows(Some(&root.id))
        .into_iter()
        .map(|(r, _)| r.id.clone())
        .collect();
    drop(store);

    let mut links = Vec::new();
    if root.id != run.id {
        if let Some(path) = existing_summary(&root.id) {
            links.push(Link {
                label: "chain write-up".to_owned(),
                kind: "markdown".to_owned(),
                target: path.to_string_lossy().into_owned(),
            });
        }
    }
    if let Some(path) = existing_summary(&run.id) {
        links.push(Link {
            label: if root.id == run.id {
                "write-up".to_owned()
            } else {
                "this stage's write-up".to_owned()
            },
            kind: "markdown".to_owned(),
            target: path.to_string_lossy().into_owned(),
        });
    }
    if let Some(ticket) = ticket_url(&run) {
        links.push(Link {
            label: "Linear ticket".to_owned(),
            kind: "url".to_owned(),
            target: ticket,
        });
    }
    // The PR. Recovered the same way retention and pr-watch recover it —
    // through agent_core, so all three agree about what a run's PR is.
    //
    // Searched across the WHOLE chain, not just this run. Only
    // linear-implement opens a PR, but the link is worth having from any
    // stage of the ticket: standing on linear-report, "where is the PR"
    // is still the question.
    if let Some(url) = chain_pr(&chain_ids) {
        links.push(Link {
            label: "Draft PR".to_owned(),
            kind: "url".to_owned(),
            target: url,
        });
    }

    if let Some(dir) = data_dir(&run.id) {
        for (file, label) in [
            ("result.md", "result.md"),
            ("context.md", "context pack"),
            ("OUTPUT.md", "investigation write-up"),
        ] {
            let path = dir.join(file);
            if path.is_file() {
                links.push(Link {
                    label: label.to_owned(),
                    kind: "markdown".to_owned(),
                    target: path.to_string_lossy().into_owned(),
                });
            }
        }
    }
    Ok(links)
}

/// Read a markdown file the UI was told about by `run_links`.
///
/// Confined to agentd's own state directory on purpose: this command takes
/// a path from the frontend, and without that check a bug (or anything
/// else able to call it) could read any file the user can.
#[tauri::command]
pub fn read_markdown(path: String) -> Result<String, AppError> {
    let path = PathBuf::from(&path);
    let root = agent_core::paths::state_dir()
        .map_err(|e| AppError::new(format!("could not resolve the agentd state dir: {e}")))?;
    let canonical = path
        .canonicalize()
        .map_err(|e| AppError::new(format!("could not open {}: {e}", path.display())))?;
    if !canonical.starts_with(&root) {
        return Err(AppError::new(format!(
            "{} is outside the agentd state directory",
            canonical.display()
        )));
    }
    std::fs::read_to_string(&canonical)
        .map_err(|e| AppError::new(format!("could not read {}: {e}", canonical.display())))
}

/// The first PR URL any stage of the chain wrote down.
///
/// `result.md` first because that is where the implement stage reports
/// it; the write-ups second, because a data dir is pruned after four
/// hours while a write-up naming an open PR is kept indefinitely — so
/// that is the copy that still answers tomorrow.
fn chain_pr(chain_ids: &[String]) -> Option<String> {
    for id in chain_ids {
        if let Some(dir) = data_dir(id) {
            if let Some(url) = agent_core::links::pr_url_in_file(&dir.join("result.md")) {
                return Some(url);
            }
        }
    }
    chain_ids
        .iter()
        .filter_map(|id| existing_summary(id))
        .find_map(|path| agent_core::links::pr_url_in_file(&path))
}

fn existing_summary(run_id: &str) -> Option<PathBuf> {
    let path = agent_core::paths::state_dir()
        .ok()?
        .join("summaries")
        .join(format!("{run_id}.md"));
    path.is_file().then_some(path)
}

fn data_dir(run_id: &str) -> Option<PathBuf> {
    Some(agent_core::paths::data_dir().ok()?.join(run_id))
}

/// Built from the run's params, which live on the row — so unlike the
/// write-ups and the data dir, this link outlives retention.
fn ticket_url(run: &Run) -> Option<String> {
    let params = run.params.as_deref()?;
    let value = serde_json::from_str::<serde_json::Value>(params).ok()?;
    let ticket = value.get("ticket")?.as_str()?.trim();
    if ticket.is_empty() {
        return None;
    }
    if ticket.starts_with("http") {
        return Some(ticket.to_owned());
    }
    Some(format!("https://linear.app/superblocks/issue/{ticket}"))
}
