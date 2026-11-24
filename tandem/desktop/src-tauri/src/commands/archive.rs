//! Archived and forgotten PRs: listing the archive, forgetting one PR,
//! and purging expired data from the cache and the disk. Split from
//! `prs.rs`.

use chrono::Utc;
use tandem_cache::{ArchiveStore, Cache};
use tandem_core::github::ArchivedPr;
use tandem_core::TandemError;
use tauri::State;

use crate::commands::parse_repo;
use crate::state::AppState;

#[tauri::command]
pub async fn list_archived_prs(
    state: State<'_, AppState>,
    repo: String,
) -> Result<Vec<ArchivedPr>, TandemError> {
    let repo = parse_repo(&repo)?;
    state.cache.lock().await.list_archived(&repo)
}

/// Drop everything cached for one PR immediately — its detail, diff,
/// local comments, agent runs (and their logs), summary and checkout.
/// Used when you leave a PR that has merged: it's done, so the app
/// stops carrying it.
#[tauri::command]
pub async fn forget_pr(
    state: State<'_, AppState>,
    repo: String,
    number: u64,
) -> Result<(), TandemError> {
    let repo = parse_repo(&repo)?;
    let purged = {
        let mut cache = state.cache.lock().await;
        cache.forget_pr(&repo, number)?
    };
    remove_purged_dirs(&purged, &state.dirs);
    Ok(())
}

/// Purge expired archive entries and their run directories on disk.
pub(crate) fn purge_expired_data(
    cache: &mut Cache,
    dirs: &crate::settings::AppDirs,
) -> Result<(), TandemError> {
    let purged = cache.purge_expired(Utc::now())?;
    remove_purged_dirs(&purged, dirs);
    Ok(())
}

/// Mirror a DB purge on disk: the run log directories, and the agent
/// checkouts named by `workspace::ensure_pr_checkout`'s scheme.
fn remove_purged_dirs(purged: &tandem_cache::PurgedRuns, dirs: &crate::settings::AppDirs) {
    for log_path in &purged.log_paths {
        let path = std::path::Path::new(log_path);
        // Only remove directories that live under our own runs dir.
        if let Some(dir) = path.parent() {
            if dir.starts_with(&dirs.runs_dir) {
                remove_dir(dir, "run");
            }
        }
    }
    for (repo, number) in &purged.repos {
        let dir = dirs
            .worktrees_dir
            .join("agent-runs")
            .join(repo)
            .join(number.to_string());
        remove_dir(&dir, "agent checkout");
    }
}

fn remove_dir(dir: &std::path::Path, kind: &str) {
    if let Err(e) = std::fs::remove_dir_all(dir) {
        if e.kind() != std::io::ErrorKind::NotFound {
            tracing::warn!(dir = %dir.display(), kind, error = %e, "failed to purge directory");
        }
    }
}
