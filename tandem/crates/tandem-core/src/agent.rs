//! Agent workflow types: how a "bring your own AI" reviewer is configured
//! and what its runs emit.
//!
//! The schema deliberately mirrors the `agentd` manifest format (agentd
//! lives in this monorepo's `workflows/`): runner / model /
//! `allowed_tools` / `append_system_prompt` / `env_file`, so specs feel
//! familiar and could be shared later.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Which backend executes the agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum RunnerKind {
    /// `claude -p --output-format stream-json` (headless Claude Code).
    ClaudeHeadless,
    /// `codex exec --json` (headless Codex).
    CodexHeadless,
    /// `grok -p <prompt> --output-format streaming-json` (headless Grok
    /// Build, xAI's coding CLI).
    GrokHeadless,
    /// Any user-supplied command; stdout treated as the event stream.
    Custom {
        /// Shell command line, run via `/bin/sh -c`; the prompt arrives on
        /// stdin.
        command: String,
    },
}

/// How the runner reaches its model provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum AuthMode {
    /// Reuse the CLI's own login (claude / codex already authenticated).
    CliSession,
    /// API key exposed to the process via `env_var`.
    ApiKey {
        /// Name of the environment variable the runner reads the key from.
        env_var: String,
    },
    /// Custom OpenAI-compatible/other endpoint.
    Endpoint {
        /// Base URL of the provider's API, exported to the runner.
        base_url: String,
        /// Variable holding the API key for `base_url`, if it needs one.
        env_var: Option<String>,
    },
}

/// A configured agent reviewer. Everything here is user-editable in the
/// app's settings UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSpec {
    /// Unique name, e.g. "claude-reviewer".
    pub name: String,
    /// Which CLI executes the agent.
    pub runner: RunnerKind,
    /// How the runner authenticates with its model provider.
    pub auth: AuthMode,
    /// Model override passed to the runner; `None` uses the CLI's default.
    pub model: Option<String>,
    /// Tool allowlist passed to the runner (claude `--allowedTools`).
    #[serde(default)]
    pub allowed_tools: Vec<String>,
    /// Extra system-prompt text appended after Tandem's review context.
    pub append_system_prompt: Option<String>,
    /// The review instructions (what to look for, tone, severity bar).
    pub prompt: String,
    /// Extra environment for the process. Values live in the OS keychain /
    /// env files, never in the cache DB.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// Network endpoints the agent may reach. Unenforced in v1 (local
    /// process); v2 enforces this via the Docker + squid sandbox — see
    /// docs/SANDBOXING.md.
    #[serde(default)]
    pub network_allowlist: Vec<String>,
    /// Hard wall-clock limit for one run, in minutes; the process is killed
    /// and the run marked [`RunStatus::TimedOut`] once it elapses.
    #[serde(default = "default_timeout_minutes")]
    pub timeout_minutes: u64,
    /// Whether a review run also writes the PR-level summary shown above
    /// the diff. On by default; turn it off for agents that should only
    /// leave inline comments.
    #[serde(default = "default_true")]
    pub summary: bool,
}

fn default_true() -> bool {
    true
}

fn default_timeout_minutes() -> u64 {
    15
}

/// Lifecycle of one agent run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    /// Spawning the process; no output yet.
    Starting,
    /// Process is up and streaming events.
    Running,
    /// Exited cleanly.
    Succeeded,
    /// Exited non-zero, failed to spawn, or was interrupted by a restart.
    Failed,
    /// Stopped by the user.
    Cancelled,
    /// Killed after [`AgentSpec::timeout_minutes`].
    TimedOut,
}

/// One event on a run's log stream. Runs are persisted as JSONL files of
/// these events (same shape agentd uses), so logs are greppable and the
/// UI can tail them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunEvent {
    /// Run this event belongs to.
    pub run_id: String,
    /// Monotonic per-run sequence number, starting at 0 — orders events
    /// within the JSONL log.
    pub seq: u64,
    /// When Tandem recorded the event.
    pub at: DateTime<Utc>,
    /// What kind of payload this is.
    pub kind: RunEventKind,
    /// Raw payload (runner JSON line, or plain text for `Raw`).
    pub payload: String,
}

/// Classifies a [`RunEvent`] so the UI can render it without re-parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunEventKind {
    /// Runner lifecycle (status changes, exit codes).
    Lifecycle,
    /// Structured JSON from the runner (assistant messages, tool use).
    Runner,
    /// A review comment the agent emitted (an `tandem_comment` line).
    Comment,
    /// The PR-level write-up the agent emitted (a `tandem_summary` line).
    Summary,
    /// Unstructured stdout/stderr.
    Raw,
}

/// Summary row for a run, persisted in the cache.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRun {
    /// Run id (UUID string); also names the run's directory on disk.
    pub run_id: String,
    /// [`AgentSpec::name`] of the agent that ran.
    pub agent_name: String,
    /// `owner/name` of the reviewed repo.
    pub repo_slug: String,
    /// PR the run reviewed.
    pub pr_number: u64,
    /// PR head SHA the run reviewed — comments it emits anchor here.
    pub head_sha: String,
    /// Current lifecycle state.
    pub status: RunStatus,
    /// When the run was spawned.
    pub started_at: DateTime<Utc>,
    /// When the run reached a terminal state; `None` while active.
    pub finished_at: Option<DateTime<Utc>>,
    /// Path to the JSONL event log on disk.
    pub log_path: String,
    /// Number of comments the run has emitted so far.
    pub comment_count: u64,
    /// What the run was for: "pr review", "thread reply",
    /// "conflict analysis" — drives notifications.
    #[serde(default = "default_purpose")]
    pub purpose: String,
    /// Local comment the run centers on (thread root / its own general
    /// comment), for jump-to-highlight.
    #[serde(default)]
    pub target_comment_id: Option<String>,
    /// Human-readable failure reason for failed/timed-out/interrupted
    /// runs — surfaced on run rows instead of a bare status.
    #[serde(default)]
    pub error: Option<String>,
}

fn default_purpose() -> String {
    "pr review".to_owned()
}
