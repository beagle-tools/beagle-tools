use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

mod enums;

pub use enums::{ConcurrencyPolicy, Isolation, NotifyPolicy, RunState, Runner, Trigger};

/// A project groups related agents — the "namespace". One directory of
/// manifests per project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectSpec {
    /// Directory name of the project; unique.
    pub name: String,
    #[serde(default)]
    /// Free text from the project's `project.yaml`, if any.
    pub description: Option<String>,
}

/// A declared agent — the "workload". This is the fully resolved form: paths
/// are absolute and the project is filled in from the manifest directory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSpec {
    /// Owning project — the manifest's parent directory name.
    pub project: String,
    /// Agent name, unique within its project.
    pub name: String,
    /// The manifest file this agent was loaded from — what the TUI's `e`
    /// (edit) action opens.
    #[serde(default)]
    pub source: PathBuf,
    /// Which CLI executes runs of this agent.
    pub runner: Runner,
    #[serde(default)]
    /// Model passed to the runner (`--model`/`-m`); `None` = the CLI's default.
    pub model: Option<String>,
    /// Standard 5-field cron expression. `None` means manual-trigger only.
    #[serde(default)]
    pub schedule: Option<String>,
    /// Absolute directory runs execute in (or branch a worktree from).
    pub workdir: PathBuf,
    /// Absolute path to the prompt handed to an LLM runner: a single file, or
    /// a directory whose `.md` files are concatenated in name order. `None`
    /// for the `script` runner.
    #[serde(default)]
    pub prompt: Option<PathBuf>,
    /// Shell command executed by the `script` runner (in the workdir).
    #[serde(default)]
    pub command: Option<String>,
    /// Claude only: tools the headless run may use without prompting,
    /// passed through as `--allowedTools`.
    #[serde(default)]
    pub allowed_tools: Option<Vec<String>>,
    /// Claude only: file whose contents are passed as
    /// `--append-system-prompt` (resolved like `prompt`).
    #[serde(default)]
    pub append_system_prompt: Option<PathBuf>,
    /// Claude only: extra directories the session may access (`--add-dir`).
    #[serde(default)]
    pub add_dirs: Vec<PathBuf>,
    /// Tools denied even when `allowed_tools` would permit them. Deny wins,
    /// so this is how you express "all of git except the mutating parts":
    /// Bash patterns are prefix matches and cannot say "except".
    pub disallowed_tools: Option<Vec<String>>,
    /// MCP servers that must be connected before a run is queued, by the
    /// name they are registered under locally (`claude mcp get <name>`).
    #[serde(default)]
    pub requires_mcp: Vec<String>,
    /// Parameters this agent accepts as a job (`agentctl -j <name> -p k=v`).
    /// Empty means free-form: any `-p` is passed through unchecked.
    #[serde(default)]
    pub params: Vec<crate::params::ParamSpec>,
    /// Pin this agent to an explicit set of MCP servers: the JSON config is
    /// passed as `--mcp-config` together with `--strict-mcp-config`, so every
    /// other configured server — including the claude.ai account connectors —
    /// is ignored for the run. `None` leaves the ambient config in place.
    pub mcp_config: Option<PathBuf>,
    /// Extra environment variables exported into the run.
    #[serde(default)]
    pub env: std::collections::BTreeMap<String, String>,
    /// Dotenv-style file sourced into the run before `env` — keeps secrets
    /// out of the (committed) manifest.
    #[serde(default)]
    pub env_file: Option<PathBuf>,
    /// Provision a persistent per-agent `SQLite` database and export its path
    /// as `$AGENTD_DB` — the durable ledger for pollers/dedup.
    #[serde(default)]
    pub database: bool,
    /// Cap on simultaneously *running* runs of this agent; excess pending
    /// runs stay queued. `None` = unlimited.
    #[serde(default)]
    pub max_concurrent: Option<u32>,
    /// Delete no-op runs (succeeded without writing an announcement) this
    /// many minutes after they finish — for high-frequency pollers whose
    /// quiet ticks are pure noise. The newest run is always kept.
    #[serde(default)]
    pub noop_retention_minutes: Option<u64>,
    #[serde(default = "default_timeout_minutes")]
    /// Wall-clock limit after which the daemon kills the run as `TimedOut`.
    /// Defaults to 30.
    pub timeout_minutes: u64,
    /// Per-agent notification override; `None` = the daemon's `--notify`
    /// default.
    #[serde(default)]
    pub notify: Option<NotifyPolicy>,
    /// Image shown in this agent's notifications (needs terminal-notifier);
    /// `None` = the daemon's default icon.
    #[serde(default)]
    pub notify_icon: Option<PathBuf>,
    /// Image for failed/timed-out/killed runs; `None` = the daemon's
    /// failure default, then `notify_icon`.
    #[serde(default)]
    pub notify_icon_failure: Option<PathBuf>,
    #[serde(default)]
    /// What a cron tick does while a previous run is still active.
    pub concurrency: ConcurrencyPolicy,
    #[serde(default)]
    /// Whether runs get their own git worktree.
    pub isolation: Isolation,
    #[serde(default = "default_true")]
    /// `false` keeps the agent loaded but never schedules it. Defaults to true.
    pub enabled: bool,
    /// Override for whether this agent is a job somebody starts, rather
    /// than a step another agent triggers. `None` means "work it out" —
    /// see `pipelines::is_entry`. Set it when the inference cannot know
    /// yet: a step that has never run looks like an entry point, because
    /// nothing has shown otherwise.
    #[serde(default)]
    pub entrypoint: Option<bool>,
}

impl AgentSpec {
    /// `project/name`, the unique key used everywhere (like a namespaced name).
    pub fn qualified_name(&self) -> String {
        format!("{}/{}", self.project, self.name)
    }
}

fn default_timeout_minutes() -> u64 {
    30
}

fn default_true() -> bool {
    true
}

/// One execution instance of an agent — the "pod".
///
/// Not `Eq`: `cost_usd` is an `f64`, and the runner's reported price is the
/// one thing here nobody should be comparing for exact equality anyway.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Run {
    /// Unique, time-sortable run id.
    pub id: String,
    /// Project of the agent this is a run of.
    pub project: String,
    /// Name of the agent this is a run of.
    pub agent: String,
    /// Where the run is in its lifecycle.
    pub state: RunState,
    /// What created the run.
    pub trigger: Trigger,
    /// The run that triggered this one (`agentctl run --parent`) — the DAG
    /// lineage edge.
    pub parent_run_id: Option<String>,
    /// Per-run parameters as a JSON object (`agentctl run --param K=V`),
    /// exported to the process as `AGENTD_PARAM_<K>`.
    pub params: Option<String>,
    /// tmux session name hosting the process — the "container" id.
    pub tmux_session: Option<String>,
    /// Raw runner output (stream-json or script stdout); set when it starts.
    pub log_path: Option<PathBuf>,
    /// Directory the run actually executed in (a worktree when isolated).
    pub workdir: Option<PathBuf>,
    /// Model override for this run only (`agentctl run --model`). Wins over
    /// the manifest's `model:`; `None` falls back to it.
    pub model: Option<String>,
    /// When the run was queued.
    pub created_at: DateTime<Utc>,
    /// When the daemon started it; `None` while pending.
    pub started_at: Option<DateTime<Utc>>,
    /// When it reached a terminal state; `None` until then.
    pub finished_at: Option<DateTime<Utc>>,
    /// The process's exit status, when it exited on its own.
    pub exit_code: Option<i32>,
    /// What the run cost, as its runner reported it. `None` when the runner
    /// does not report a price (codex) or the run never finished.
    pub cost_usd: Option<f64>,
    /// Agent turns the runner reported, when it reports them.
    pub num_turns: Option<i64>,
    /// The runner's raw usage object (tokens, cache reads), kept whole.
    pub usage_json: Option<String>,
}

/// A single observed event within a run (a parsed stream-json line, a state
/// change, a daemon note). The TUI's live feed is built from these.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    /// Monotonic row id; tail a run by polling for ids greater than the last seen.
    pub id: i64,
    /// The run this event belongs to.
    pub run_id: String,
    /// When the event was recorded.
    pub ts: DateTime<Utc>,
    /// Short machine-readable kind, e.g. `state`, `tool_use`, `message`, `raw`.
    pub kind: String,
    /// The raw line or note, capped at 4 KB by the daemon.
    pub payload: Option<String>,
}

#[cfg(test)]
// Tests should fail loudly on a broken fixture; unwrapping is the clearest way.
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn enums_round_trip_through_strings() {
        for state in [
            RunState::Pending,
            RunState::Running,
            RunState::Succeeded,
            RunState::Failed,
            RunState::TimedOut,
            RunState::Killed,
        ] {
            assert_eq!(state.as_str().parse::<RunState>().unwrap(), state);
        }
        assert_eq!("claude".parse::<Runner>().unwrap(), Runner::Claude);
        assert_eq!("manual".parse::<Trigger>().unwrap(), Trigger::Manual);
    }

    #[test]
    fn terminal_states() {
        assert!(!RunState::Pending.is_terminal());
        assert!(!RunState::Running.is_terminal());
        assert!(RunState::Succeeded.is_terminal());
        assert!(RunState::TimedOut.is_terminal());
    }
}
