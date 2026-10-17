//! The orchestrator: the supervised state machine that ties the engine
//! together. One `run_once` tick prechecks auth, enqueues the triggered RCAs,
//! and processes a bounded batch of them concurrently — each in its own
//! worktree and process group.
//!
//! Deterministic Rust owns every risky step. For each actionable RCA it creates
//! an isolated worktree, runs a headless `claude` session that implements
//! `remediation.md` and commits, then (only if the session succeeded and
//! actually committed) pushes and opens a PR, attaches it to the RCA, and
//! advances the RCA to `final-review`. The worktree is always torn down, so a
//! failure never leaks a checkout; every job runs inside `catch_unwind`, so a
//! panic in one job is isolated, logged as a failure, and never takes the
//! daemon down. Failures below the retry cap return the job to `pending`;
//! at the cap they are marked `failed`.

use std::panic::{self, AssertUnwindSafe};
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use beagle::model::{RcaId, Status};
use beagle::store::Store as RcaStore;

use crate::poller::Poller;
use crate::runner::{Runner, SessionSpec};
use crate::store::{Actionable, Store as JobStore};
use crate::worktree::{Manager as Worktrees, Worktree};

mod publish;

pub use publish::{default_precheck, GhPublisher, PrRequest, PublishError, Publisher};

/// The tunables and per-agent context for one orchestrator.
#[derive(Debug)]
pub struct RunPolicy {
    /// The RCA status that triggers work (e.g. `agent`).
    pub trigger_status: String,
    /// The base prompt (already read from the agent's prompt file).
    pub base_prompt: String,
    /// The `--allowedTools` list for the session.
    pub allowed_tools: Vec<String>,
    /// The per-session hard timeout.
    pub timeout: Duration,
    /// The most sessions to run at once.
    pub max_concurrent: usize,
    /// The most jobs to launch in a single tick.
    pub max_per_poll: usize,
    /// The attempt cap before a job is marked failed.
    pub max_attempts: u32,
    /// Where per-session logs are written.
    pub logs_dir: PathBuf,
}

/// The result of processing one job in a tick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobOutcome {
    /// A PR was opened; `warning` carries any non-fatal post-publish issue
    /// (e.g. the RCA status could not be advanced).
    Published {
        /// The opened PR URL.
        pr_url: String,
        /// A non-fatal warning, if any.
        warning: Option<String>,
    },
    /// The job failed but is under the retry cap and was returned to `pending`.
    WillRetry {
        /// Why it failed.
        reason: String,
    },
    /// The job failed at the retry cap and was marked `failed`.
    GaveUp {
        /// Why it failed.
        reason: String,
    },
}

/// One job's identity and outcome within a tick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobResult {
    /// The RCA id.
    pub id: String,
    /// What happened.
    pub outcome: JobOutcome,
}

/// The result of a single [`Orchestrator::run_once`] tick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tick {
    /// The precheck blocked work; nothing ran.
    Skipped {
        /// Why the tick idle-skipped.
        reason: String,
    },
    /// Work ran; one entry per processed job.
    Ran {
        /// Per-job results.
        results: Vec<JobResult>,
    },
}

/// The supervised orchestrator for one agent.
pub struct Orchestrator {
    policy: RunPolicy,
    rcas: RcaStore,
    jobs: Mutex<JobStore>,
    worktrees: Worktrees,
    runner: Runner,
    publisher: Box<dyn Publisher>,
    precheck: Box<dyn Fn() -> Option<String> + Send + Sync>,
}

// Manual: the publisher and precheck are trait objects with no Debug bound.
impl std::fmt::Debug for Orchestrator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Orchestrator")
            .field("policy", &self.policy)
            .field("rcas", &self.rcas)
            .field("jobs", &self.jobs)
            .field("worktrees", &self.worktrees)
            .field("runner", &self.runner)
            .finish_non_exhaustive()
    }
}

impl Orchestrator {
    /// Assembles an orchestrator from its collaborators. `precheck` returns
    /// `Some(reason)` when the daemon should idle-skip (auth not ready).
    #[must_use]
    pub fn new(
        policy: RunPolicy,
        rcas: RcaStore,
        jobs: JobStore,
        worktrees: Worktrees,
        runner: Runner,
        publisher: Box<dyn Publisher>,
        precheck: Box<dyn Fn() -> Option<String> + Send + Sync>,
    ) -> Self {
        Self {
            policy,
            rcas,
            jobs: Mutex::new(jobs),
            worktrees,
            runner,
            publisher,
            precheck,
        }
    }

    /// Runs one tick: precheck, enqueue triggered RCAs, then process a bounded
    /// batch concurrently. Never panics — a job that panics is isolated and
    /// reported as a failure.
    #[must_use]
    pub fn run_once(&self) -> Tick {
        if let Some(reason) = (self.precheck)() {
            return Tick::Skipped { reason };
        }

        let poller = Poller::new(&self.rcas, &self.policy.trigger_status);
        if let Err(err) = poller.enqueue(&guard(&self.jobs)) {
            return Tick::Ran {
                results: vec![fatal(format!("enqueue failed: {err}"))],
            };
        }

        let batch = match guard(&self.jobs).select_actionable(self.policy.max_attempts) {
            Ok(mut jobs) => {
                jobs.truncate(self.policy.max_per_poll);
                jobs
            }
            Err(err) => {
                return Tick::Ran {
                    results: vec![fatal(format!("select failed: {err}"))],
                };
            }
        };
        let worker_count = self.policy.max_concurrent.max(1).min(batch.len());
        if batch.is_empty() {
            return Tick::Ran {
                results: Vec::new(),
            };
        }

        let queue = Mutex::new(batch);
        let results = Mutex::new(Vec::new());
        std::thread::scope(|scope| {
            for _ in 0..worker_count {
                scope.spawn(|| loop {
                    let job = guard(&queue).pop();
                    let Some(job) = job else { break };
                    let result = self.supervise(&job);
                    guard(&results).push(result);
                });
            }
        });
        let collected = guard(&results).drain(..).collect();
        Tick::Ran { results: collected }
    }

    /// Processes one job with full supervision: count the attempt, mark it
    /// processing, run it inside `catch_unwind`, then record the outcome.
    fn supervise(&self, job: &Actionable) -> JobResult {
        let attempts = match guard(&self.jobs).increment_attempts(&job.id) {
            Ok(attempts) => attempts,
            Err(err) => {
                return JobResult {
                    id: job.id.clone(),
                    outcome: JobOutcome::GaveUp {
                        reason: format!("increment_attempts: {err}"),
                    },
                };
            }
        };
        if let Err(err) = guard(&self.jobs).mark_processing(&job.id) {
            return JobResult {
                id: job.id.clone(),
                outcome: JobOutcome::GaveUp {
                    reason: format!("mark_processing: {err}"),
                },
            };
        }

        let processed = panic::catch_unwind(AssertUnwindSafe(|| self.process_job(job)));
        let outcome = match processed {
            Ok(Ok(pr_url)) => self.on_success(&job.id, pr_url),
            Ok(Err(reason)) => self.on_failure(&job.id, attempts, reason),
            Err(_) => self.on_failure(&job.id, attempts, "agent run panicked".to_string()),
        };
        JobResult {
            id: job.id.clone(),
            outcome,
        }
    }

    /// The happy path: attach the PR and advance the RCA (best-effort — a
    /// post-publish error is a warning, not a failure, since the PR is open),
    /// then mark the job done.
    fn on_success(&self, id: &str, pr_url: String) -> JobOutcome {
        let warning = self.record_success(id, &pr_url).err();
        let _ = guard(&self.jobs).mark_done(id, Some(&pr_url));
        JobOutcome::Published { pr_url, warning }
    }

    /// Attaches the PR to the RCA and advances it to `final-review`.
    fn record_success(&self, id: &str, pr_url: &str) -> Result<(), String> {
        let rca_id = RcaId::new(id).map_err(|err| err.to_string())?;
        self.rcas
            .add_pr(&rca_id, pr_url)
            .map_err(|err| err.to_string())?;
        self.rcas
            .set_status(&rca_id, Status::FinalReview)
            .map_err(|err| err.to_string())?;
        Ok(())
    }

    /// The failure path: retry below the cap, give up at it.
    fn on_failure(&self, id: &str, attempts: u32, reason: String) -> JobOutcome {
        if attempts < self.policy.max_attempts {
            let _ = guard(&self.jobs).mark_pending(id);
            JobOutcome::WillRetry { reason }
        } else {
            let _ = guard(&self.jobs).mark_failed(id, "run", &reason);
            JobOutcome::GaveUp { reason }
        }
    }

    /// Creates a worktree, runs the session in it, and always tears it down.
    fn process_job(&self, job: &Actionable) -> Result<String, String> {
        let worktree = self
            .worktrees
            .create(&job.id)
            .map_err(|err| err.to_string())?;
        let outcome = self.run_in_worktree(job, &worktree);
        let _ = self.worktrees.remove(&worktree);
        outcome
    }

    /// Runs the session, verifies it committed, and publishes the PR.
    fn run_in_worktree(&self, job: &Actionable, worktree: &Worktree) -> Result<String, String> {
        let poller = Poller::new(&self.rcas, &self.policy.trigger_status);
        let remediation = poller
            .read_remediation(&job.id)
            .map_err(|err| err.to_string())?;
        let prompt = self.compose_prompt(&job.id, job.title.as_deref(), remediation.as_deref());
        let log_path = self.policy.logs_dir.join(format!("{}.log", job.id));
        let outcome = self
            .runner
            .run(
                SessionSpec {
                    working_dir: &worktree.path,
                    prompt: &prompt,
                    allowed_tools: &self.policy.allowed_tools,
                    log_path: &log_path,
                },
                self.policy.timeout,
            )
            .map_err(|err| err.to_string())?;
        if !outcome.is_success() {
            return Err(format!("claude session did not succeed: {outcome:?}"));
        }
        if !self
            .worktrees
            .has_commits(worktree)
            .map_err(|err| err.to_string())?
        {
            return Err("claude made no commits".to_string());
        }
        self.publisher
            .publish(&PrRequest {
                rca_id: &job.id,
                title: job.title.as_deref(),
                worktree,
            })
            .map_err(|err| err.to_string())
    }

    /// Composes the full prompt: the base prompt plus the RCA's identity and
    /// `remediation.md`.
    fn compose_prompt(&self, id: &str, title: Option<&str>, remediation: Option<&str>) -> String {
        let mut prompt = self.policy.base_prompt.clone();
        prompt.push_str("\n\n---\nRCA id: ");
        prompt.push_str(id);
        prompt.push('\n');
        if let Some(title) = title {
            prompt.push_str("Title: ");
            prompt.push_str(title);
            prompt.push('\n');
        }
        prompt.push_str("\n## remediation.md\n\n");
        prompt.push_str(remediation.unwrap_or("(no remediation.md found)"));
        prompt
    }
}

/// Recovers a mutex even if a previous holder panicked.
fn guard<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A single-entry result list for a whole-tick fatal error.
fn fatal(reason: String) -> JobResult {
    JobResult {
        id: String::new(),
        outcome: JobOutcome::GaveUp { reason },
    }
}

#[cfg(test)]
#[path = "../tests/orchestrator.rs"]
mod tests;
