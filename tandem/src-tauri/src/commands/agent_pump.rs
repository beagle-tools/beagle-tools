//! Consume an agent run's event stream: persist comments, keep the run
//! row current, and forward everything to the frontend. Split out of
//! agents.rs to stay under the 400-line file cap.

use tandem_agents::events::{extract_embedded_summary, parse_comment, parse_summary};
use tandem_cache::{ReviewStore, SummaryStore};
use tandem_core::agent::{AgentRun, AgentSpec, RunEvent, RunEventKind, RunStatus};
use tandem_core::github::RepoRef;
use tandem_core::review::{
    CommentAuthorKind, CommentSeverity, DiffSide, NewLocalComment, PrSummary,
};
use tauri::{AppHandle, Emitter, Manager};

use crate::state::AppState;

pub(crate) async fn pump_events(
    app: AppHandle,
    mut events: tokio::sync::mpsc::UnboundedReceiver<RunEvent>,
    mut run: AgentRun,
    spec: AgentSpec,
    repo: RepoRef,
    head_sha: String,
) {
    while let Some(event) = events.recv().await {
        if let Err(e) = app.emit("tandem://agent-event", &event) {
            tracing::warn!(error = %e, "failed to forward agent event");
        }
        match event.kind {
            RunEventKind::Comment => {
                handle_comment(&app, &event, &mut run, &spec, &repo, &head_sha).await;
            }
            RunEventKind::Summary => {
                if let Some(parsed) = parse_summary(&event.payload) {
                    store_summary(&app, &parsed.body, &run, &spec, &repo, &head_sha).await;
                }
            }
            RunEventKind::Lifecycle => {
                if let Some((status, detail)) = parse_lifecycle_status(&event.payload) {
                    run.status = status;
                    if matches!(status, RunStatus::Failed | RunStatus::TimedOut) {
                        run.error = Some(detail);
                    }
                    if matches!(
                        status,
                        RunStatus::Succeeded
                            | RunStatus::Failed
                            | RunStatus::Cancelled
                            | RunStatus::TimedOut
                    ) {
                        run.finished_at = Some(chrono::Utc::now());
                    }
                    persist_run(&app, &run).await;
                }
            }
            RunEventKind::Runner | RunEventKind::Raw => {
                // Headless runners wrap model text in envelopes, so the
                // summary usually arrives in here rather than as a bare
                // stdout line (same as comments).
                if spec.summary {
                    if let Some(parsed) = extract_embedded_summary(&event.payload) {
                        store_summary(&app, &parsed.body, &run, &spec, &repo, &head_sha).await;
                    }
                }
            }
        }
    }
    // Stream closed: drop the cancel handle for this run.
    let state = app.state::<AppState>();
    state.runs.lock().await.remove(&run.run_id);
}

/// Upsert the PR's summary and tell the UI. Skipped when the agent has
/// summaries turned off, so a spec opt-out holds even if the model
/// emits one anyway.
async fn store_summary(
    app: &AppHandle,
    body: &str,
    run: &AgentRun,
    spec: &AgentSpec,
    repo: &RepoRef,
    head_sha: &str,
) {
    if !spec.summary {
        return;
    }
    let state = app.state::<AppState>();
    let result = {
        let cache = state.cache.lock().await;
        // A re-review rewrites the body; the discussion hanging off it
        // carries over.
        let thread_root_id = cache
            .get_pr_summary(repo, run.pr_number)
            .ok()
            .flatten()
            .and_then(|s| s.thread_root_id);
        let summary = PrSummary {
            repo: repo.clone(),
            pr_number: run.pr_number,
            body: body.to_owned(),
            agent_name: spec.name.clone(),
            run_id: run.run_id.clone(),
            head_sha: head_sha.to_owned(),
            updated_at: chrono::Utc::now(),
            thread_root_id,
        };
        cache.put_pr_summary(&summary).map(|()| summary)
    };
    match result {
        Ok(summary) => {
            if let Err(e) = app.emit("tandem://summary-updated", &summary) {
                tracing::warn!(error = %e, "failed to emit summary-updated");
            }
        }
        Err(e) => tracing::error!(error = %e, "failed to store PR summary"),
    }
}

async fn handle_comment(
    app: &AppHandle,
    event: &RunEvent,
    run: &mut AgentRun,
    spec: &AgentSpec,
    repo: &RepoRef,
    head_sha: &str,
) {
    let Some(parsed) = parse_comment(&event.payload) else {
        tracing::warn!(payload = %event.payload, "agent emitted malformed tandem_comment");
        return;
    };
    // A comment without a valid line can't anchor in the diff, and one
    // without a path can't anchor anywhere — fold either case into a
    // true PR-level (general) comment so it always renders.
    let general = parsed.line == 0 || parsed.path.is_empty();
    let new = NewLocalComment {
        repo: repo.clone(),
        pr_number: run.pr_number,
        head_sha: head_sha.to_owned(),
        path: if general { String::new() } else { parsed.path },
        side: if parsed.side == "old" {
            DiffSide::Old
        } else {
            DiffSide::New
        },
        line: if general { 0 } else { parsed.line },
        end_line: if general { None } else { parsed.end_line },
        body: parsed.body,
        suggestion: parsed.suggestion.clone(),
        author_kind: CommentAuthorKind::Agent,
        author_name: spec.name.clone(),
        severity: parse_severity(&parsed.severity),
        run_id: Some(run.run_id.clone()),
        // Conflict runs own their own section in the UI; tagging here
        // means a model that forgets the tag field can't hide them in
        // the general comment list.
        tag: parsed.tag.clone().or_else(|| {
            run.purpose
                .starts_with("conflict")
                .then(|| "conflict".to_owned())
        }),
        parent_id: parsed.parent_id.clone(),
        github_comment_id: None,
    };
    let state = app.state::<AppState>();
    let result = { state.cache.lock().await.add_comment(new) };
    match result {
        Ok(created) => {
            run.comment_count += 1;
            // Non-review runs center on their first comment.
            if run.target_comment_id.is_none() && run.purpose != "pr review" {
                run.target_comment_id = Some(created.id);
            }
            persist_run(app, run).await;
            let payload = serde_json::json!({ "repo": repo.slug(), "number": run.pr_number });
            if let Err(e) = app.emit("tandem://comments-updated", payload) {
                tracing::warn!(error = %e, "failed to emit comments-updated");
            }
        }
        Err(e) => tracing::error!(error = %e, "failed to store agent comment"),
    }
}

pub(crate) async fn persist_run(app: &AppHandle, run: &AgentRun) {
    let state = app.state::<AppState>();
    let result = { state.cache.lock().await.put_agent_run(run) };
    if let Err(e) = result {
        tracing::error!(error = %e, "failed to persist agent run");
    }
    if let Err(e) = app.emit("tandem://run-updated", run) {
        tracing::warn!(error = %e, "failed to emit run-updated");
    }
}

fn parse_lifecycle_status(payload: &str) -> Option<(RunStatus, String)> {
    #[derive(serde::Deserialize)]
    struct Lifecycle {
        status: RunStatus,
        #[serde(default)]
        detail: String,
    }
    serde_json::from_str::<Lifecycle>(payload)
        .ok()
        .map(|l| (l.status, l.detail))
}

fn parse_severity(s: &str) -> CommentSeverity {
    match s {
        "info" => CommentSeverity::Info,
        "issue" => CommentSeverity::Issue,
        "blocker" => CommentSeverity::Blocker,
        _ => CommentSeverity::Suggestion,
    }
}
