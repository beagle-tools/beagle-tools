//! Pull-request data commands: cache-first reads plus explicit syncs.
//!
//! The UI pattern is: call the `get_*` command for an instant (possibly
//! stale) render, kick off the matching `sync_*` command, and re-render
//! when it resolves. `tandem://sync` events drive the loading indicators.

use chrono::{DateTime, Utc};
use serde::Serialize;
use tandem_cache::{ArchiveStore, Cache, ReviewStore, ReviewedPrs, SummaryStore};
use tandem_core::diff::FileDiff;
use tandem_core::github::{ArchivedPr, PrDetail, PrState, PullRequest};
use tandem_core::review::{LocalComment, PrSummary};
use tandem_core::TandemError;
use tauri::{AppHandle, State};

use crate::commands::{emit_sync, parse_repo, SyncPhase};
use crate::state::AppState;
use tandem_github::PrCommit;

/// Everything the PR screen needs, served from cache in one call.
#[derive(Debug, Clone, Serialize)]
pub struct PrBundle {
    pub detail: PrDetail,
    pub diff: Vec<FileDiff>,
    pub comments: Vec<LocalComment>,
    /// The agent-written overview, when a review has produced one.
    pub summary: Option<PrSummary>,
}

#[tauri::command]
pub async fn get_pull_requests(
    state: State<'_, AppState>,
    repo: String,
) -> Result<Vec<PullRequest>, TandemError> {
    let repo = parse_repo(&repo)?;
    state.cache.lock().await.get_pull_requests(&repo)
}

/// One synced page of PRs plus whether another page likely exists.
#[derive(Debug, Clone, Serialize)]
pub struct PrPage {
    pub prs: Vec<PullRequest>,
    pub has_more: bool,
}

#[tauri::command]
pub async fn sync_pull_requests(
    app: AppHandle,
    state: State<'_, AppState>,
    repo: String,
    page: Option<u32>,
) -> Result<PrPage, TandemError> {
    let repo = parse_repo(&repo)?;
    let page = page.unwrap_or(1).max(1);
    let key = format!("prs:{}", repo.slug());
    emit_sync(&app, &key, SyncPhase::Started, None);

    let result = async {
        let client = state.github_client().await?;
        let prs = client.list_pull_requests(&repo, page).await?;

        // PRs that vanished from page 1 of the open list may have merged:
        // check each and archive merged ones (3-day retention).
        let vanished: Vec<u64> = if page == 1 {
            let cache = state.cache.lock().await;
            let open: std::collections::HashSet<u64> = prs.iter().map(|p| p.number).collect();
            cache
                .get_pull_requests(&repo)?
                .iter()
                .map(|p| p.number)
                .filter(|n| !open.contains(n))
                .collect()
        } else {
            Vec::new()
        };
        for number in vanished {
            if let Ok(pr) = client.pull_request(&repo, number).await {
                if pr.state == PrState::Merged {
                    let cache = state.cache.lock().await;
                    cache.archive_pr(&pr, Utc::now())?;
                }
            }
        }

        let mut cache = state.cache.lock().await;
        if page == 1 {
            cache.put_pull_requests(&repo, &prs)?;
        } else {
            cache.append_pull_requests(&repo, &prs)?;
        }
        purge_expired_data(&mut cache, &state.dirs)?;
        let has_more = prs.len() == tandem_github::GithubClient::PR_PAGE_SIZE;
        Ok::<_, TandemError>(PrPage { prs, has_more })
    }
    .await;

    match &result {
        Ok(_) => emit_sync(&app, &key, SyncPhase::Finished, None),
        Err(e) => emit_sync(&app, &key, SyncPhase::Error, Some(e.to_string())),
    }
    result
}

#[tauri::command]
pub async fn get_pr_bundle(
    state: State<'_, AppState>,
    repo: String,
    number: u64,
) -> Result<Option<PrBundle>, TandemError> {
    let repo = parse_repo(&repo)?;
    let cache = state.cache.lock().await;
    load_bundle(&cache, &repo, number)
}

#[tauri::command]
pub async fn sync_pr_bundle(
    app: AppHandle,
    state: State<'_, AppState>,
    repo: String,
    number: u64,
) -> Result<PrBundle, TandemError> {
    let repo = parse_repo(&repo)?;
    let key = format!("pr:{}#{number}", repo.slug());
    emit_sync(&app, &key, SyncPhase::Started, None);

    let result = async {
        let client = state.github_client().await?;
        let detail = client.pull_request_detail(&repo, number).await?;
        let raw = client.pull_request_diff_raw(&repo, number).await?;
        let diff = tandem_core::diff_parse::parse_unified_diff(&raw)?;

        let cache = state.cache.lock().await;
        cache.put_pr_detail(&repo, &detail)?;
        cache.put_diff(&repo, number, &detail.pull_request.head_sha, &raw, &diff)?;
        cache.touch_sync(&key)?;
        if detail.pull_request.state == PrState::Merged {
            cache.archive_pr(&detail.pull_request, Utc::now())?;
        }
        let comments = cache.list_comments(&repo, number)?;
        let summary = cache.get_pr_summary(&repo, number)?;
        Ok::<_, TandemError>(PrBundle {
            detail,
            diff,
            comments,
            summary,
        })
    }
    .await;

    match &result {
        Ok(_) => emit_sync(&app, &key, SyncPhase::Finished, None),
        Err(e) => emit_sync(&app, &key, SyncPhase::Error, Some(e.to_string())),
    }
    result
}

/// Search all open PRs server-side; results are merged into the cache
/// so they can be opened like any listed PR.
#[tauri::command]
pub async fn search_prs(
    app: AppHandle,
    state: State<'_, AppState>,
    repo: String,
    query: String,
) -> Result<Vec<PullRequest>, TandemError> {
    let repo = parse_repo(&repo)?;
    let key = format!("prs:{}", repo.slug());
    emit_sync(&app, &key, SyncPhase::Started, None);
    let result = async {
        let client = state.github_client().await?;
        let prs = client.search_open_prs(&repo, &query).await?;
        state.cache.lock().await.append_pull_requests(&repo, &prs)?;
        Ok::<_, TandemError>(prs)
    }
    .await;
    match &result {
        Ok(_) => emit_sync(&app, &key, SyncPhase::Finished, None),
        Err(e) => emit_sync(&app, &key, SyncPhase::Error, Some(e.to_string())),
    }
    result
}

/// Account-wide PR inbox: PRs across all repos where you're wanted.
#[tauri::command]
pub async fn list_my_prs(
    state: State<'_, AppState>,
    scope: String,
    repo: Option<String>,
) -> Result<Vec<PullRequest>, TandemError> {
    let client = state.github_client().await?;
    let base = match scope.as_str() {
        "requested" => "review-requested:@me",
        "mentions" => "mentions:@me",
        "authored" => "author:@me",
        "involved" => "involves:@me",
        "approved" => return client.approved_by_me(repo.as_deref()).await,
        other => {
            return Err(TandemError::Config(format!("unknown inbox scope: {other}")));
        }
    };
    let query = match &repo {
        Some(slug) => format!("{base} repo:{slug}"),
        None => base.to_owned(),
    };
    client.search_global_prs(&query).await
}

/// Failing check names for the hover tip on red-edged rows.
#[tauri::command]
pub async fn list_failing_checks(
    state: State<'_, AppState>,
    repo: String,
    number: u64,
) -> Result<Vec<String>, TandemError> {
    let repo = parse_repo(&repo)?;
    let client = state.github_client().await?;
    client.failing_checks(&repo, number).await
}

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

#[tauri::command]
pub async fn get_last_synced(
    state: State<'_, AppState>,
    key: String,
) -> Result<Option<DateTime<Utc>>, TandemError> {
    state.cache.lock().await.last_synced(&key)
}

fn load_bundle(
    cache: &Cache,
    repo: &tandem_core::github::RepoRef,
    number: u64,
) -> Result<Option<PrBundle>, TandemError> {
    let Some(detail) = cache.get_pr_detail(repo, number)? else {
        return Ok(None);
    };
    let diff = cache
        .get_diff(repo, number, &detail.pull_request.head_sha)?
        .unwrap_or_default();
    let comments = cache.list_comments(repo, number)?;
    let summary = cache.get_pr_summary(repo, number)?;
    Ok(Some(PrBundle {
        detail,
        diff,
        comments,
        summary,
    }))
}

/// Commits on a PR, newest last (as GitHub returns them).
#[tauri::command]
pub async fn list_pr_commits(
    state: State<'_, AppState>,
    repo: String,
    number: u64,
) -> Result<Vec<PrCommit>, TandemError> {
    let repo = parse_repo(&repo)?;
    let client = state.github_client().await?;
    client.pr_commits(&repo, number).await
}

/// How many reviewed PRs we'll follow. Past a dozen this stops being a
/// short list you act on and starts being a second inbox.
const REVIEWED_LIMIT: u32 = 12;

/// PRs you've run an agent review on, still open, that GitHub isn't
/// already asking you to review. They ride along in the requested tab
/// until they merge, so following a PR you looked at doesn't mean
/// adding yourself as a reviewer.
#[tauri::command]
pub async fn list_reviewed_prs(
    state: State<'_, AppState>,
) -> Result<Vec<PullRequest>, TandemError> {
    let candidates = {
        let cache = state.cache.lock().await;
        let mut out = Vec::new();
        for (slug, number) in cache.list_reviewed_prs(REVIEWED_LIMIT)? {
            let Ok(repo) = parse_repo(&slug) else {
                continue;
            };
            // A PR already known to be finished needs no round trip —
            // and must not come back either way.
            let cached = cache.get_pr_detail(&repo, number)?;
            if cached.is_some_and(|d| d.pull_request.state != PrState::Open) {
                continue;
            }
            out.push((repo, number));
        }
        out
    };

    let client = state.github_client().await?;
    let mut prs = Vec::new();
    for (repo, number) in candidates {
        // One at a time on purpose: this is a dozen calls at most, and
        // the requested tab already fans out hard enough.
        match client.pull_request(&repo, number).await {
            Ok(pr) if pr.state == PrState::Open => prs.push(pr),
            Ok(_) => {}
            Err(e) => {
                tracing::warn!(repo = %repo.slug(), number, error = %e, "reviewed PR refresh failed");
            }
        }
    }
    Ok(prs)
}
