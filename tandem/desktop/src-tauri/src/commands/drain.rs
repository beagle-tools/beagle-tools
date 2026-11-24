//! Draining the review queue another process fills.
//!
//! The outside asks for a review; Tandem performs it, with the agent
//! config, prompts and conventions it already has. That split is the
//! point: a cron job decides *what* needs reviewing, and never has to
//! know *how* this user reviews.

use std::time::Duration;

use tandem_cache::{ReviewQueue, ReviewStore};
use tauri::{AppHandle, Manager};

use crate::commands::parse_repo;
use crate::state::AppState;

/// Often enough that a queued review starts while you're still looking
/// at the app, rarely enough to be invisible.
const EVERY: Duration = Duration::from_secs(20);

/// Watch the queue for as long as the app is running. Jobs queued while
/// it was closed are picked up on the first pass.
pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            if let Err(e) = drain_once(&app).await {
                tracing::warn!(error = %e, "review queue drain failed");
            }
            tokio::time::sleep(EVERY).await;
        }
    });
}

async fn drain_once(app: &AppHandle) -> Result<(), tandem_core::TandemError> {
    let state = app.state::<AppState>();
    let jobs = { state.cache.lock().await.pending_review_jobs()? };
    for job in jobs {
        let outcome = start(app, &job).await;
        let cache = state.cache.lock().await;
        match outcome {
            Ok(run_id) => {
                tracing::info!(repo = %job.repo, number = job.number, requested_by = %job.requested_by, "started a queued review");
                cache.finish_review_job(&job.id, Some(&run_id), None)?;
            }
            // Marked done either way: a job that can't run is a job to
            // report, not to retry forever against the same error.
            Err(e) => {
                tracing::warn!(repo = %job.repo, number = job.number, error = %e, "queued review failed to start");
                cache.finish_review_job(&job.id, None, Some(&e.to_string()))?;
            }
        }
    }
    Ok(())
}

async fn start(
    app: &AppHandle,
    job: &tandem_cache::ReviewJob,
) -> Result<String, tandem_core::TandemError> {
    let state = app.state::<AppState>();
    let repo = parse_repo(&job.repo)?;

    // Whichever agent was asked for, else the first configured one —
    // the same choice the review button's picker defaults to.
    let agent = if let Some(name) = &job.agent {
        name.clone()
    } else {
        let specs = state.cache.lock().await.list_agent_specs()?;
        specs
            .first()
            .map(|s| s.name.clone())
            .ok_or_else(|| tandem_core::TandemError::Agent("no agents configured".into()))?
    };

    // A review needs the detail and diff cached; a PR nobody has opened
    // has neither, which is the normal case for a queued job.
    let needs_sync = {
        let cache = state.cache.lock().await;
        match cache.get_pr_detail(&repo, job.number)? {
            None => true,
            Some(detail) => cache
                .get_raw_diff(&repo, job.number, &detail.pull_request.head_sha)?
                .is_none(),
        }
    };
    if needs_sync {
        crate::commands::prs::sync_pr_bundle(
            app.clone(),
            state.clone(),
            job.repo.clone(),
            job.number,
        )
        .await?;
    }

    crate::commands::agents::start_agent_review(
        app.clone(),
        state.clone(),
        agent,
        job.repo.clone(),
        job.number,
    )
    .await
}
