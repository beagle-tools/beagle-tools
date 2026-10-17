//! The small closed vocabularies of the domain: runners, run states,
//! triggers, and the per-agent policies. Each serialises lowercase, the same
//! spelling manifests use and the store persists.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::CoreError;

/// Which CLI executes the run — the "brain" behind an agent. `Script` is the
/// deterministic one: a shell command instead of an LLM, used for DAG nodes
/// that poll/branch/fan out (they trigger other agents via `agentctl run`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Runner {
    /// Anthropic's `claude -p` headless CLI.
    Claude,
    /// The `codex exec` CLI.
    Codex,
    /// A plain shell command (`command:`), no LLM.
    Script,
}

impl Runner {
    /// Lowercase name, as written in manifests and stored in the database.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Script => "script",
        }
    }
}

impl fmt::Display for Runner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Runner {
    type Err = CoreError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "claude" => Ok(Self::Claude),
            "codex" => Ok(Self::Codex),
            "script" => Ok(Self::Script),
            other => Err(CoreError::UnknownVariant {
                kind: "runner",
                value: other.to_owned(),
            }),
        }
    }
}

/// Lifecycle state of a [`Run`](super::Run) (the "pod phase").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RunState {
    /// Queued; the daemon has not started it yet.
    Pending,
    /// Its tmux session is (or was last seen) alive.
    Running,
    /// Exited 0.
    Succeeded,
    /// Exited non-zero, or its session vanished without an exit marker.
    Failed,
    /// Killed by the daemon after `timeout_minutes`.
    TimedOut,
    /// Stopped by a person (agentctl, agenttop, or Agentdesk).
    Killed,
}

impl RunState {
    /// Lowercase name as stored in the database (`timedout`, not `timed_out`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::TimedOut => "timedout",
            Self::Killed => "killed",
        }
    }

    /// True once the run can no longer change state.
    pub fn is_terminal(self) -> bool {
        !matches!(self, Self::Pending | Self::Running)
    }
}

impl fmt::Display for RunState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for RunState {
    type Err = CoreError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(Self::Pending),
            "running" => Ok(Self::Running),
            "succeeded" => Ok(Self::Succeeded),
            "failed" => Ok(Self::Failed),
            "timedout" => Ok(Self::TimedOut),
            "killed" => Ok(Self::Killed),
            other => Err(CoreError::UnknownVariant {
                kind: "run state",
                value: other.to_owned(),
            }),
        }
    }
}

/// What caused a run to be created.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Trigger {
    /// The agent's `schedule:` fired.
    Cron,
    /// Someone (or another run, via `agentctl run`) asked for it.
    Manual,
}

impl Trigger {
    /// Lowercase name as stored in the database.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cron => "cron",
            Self::Manual => "manual",
        }
    }
}

impl fmt::Display for Trigger {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Trigger {
    type Err = CoreError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "cron" => Ok(Self::Cron),
            "manual" => Ok(Self::Manual),
            other => Err(CoreError::UnknownVariant {
                kind: "trigger",
                value: other.to_owned(),
            }),
        }
    }
}

/// What to do when a cron tick fires while a previous run is still going.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConcurrencyPolicy {
    /// Skip the tick (the default, and almost always what you want).
    #[default]
    Forbid,
    /// Launch anyway; runs pile up side by side.
    Allow,
}

/// When to send a macOS notification about a finished run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NotifyPolicy {
    /// Every terminal state.
    All,
    /// Failed / timed-out / killed only.
    Failures,
    /// Never notify.
    Off,
}

impl FromStr for NotifyPolicy {
    type Err = CoreError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "all" => Ok(Self::All),
            "failures" => Ok(Self::Failures),
            "off" => Ok(Self::Off),
            other => Err(CoreError::UnknownVariant {
                kind: "notify policy",
                value: other.to_owned(),
            }),
        }
    }
}

/// Where the run executes relative to the agent's workdir.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Isolation {
    /// Run directly in the declared workdir.
    #[default]
    None,
    /// Run in a fresh `git worktree` of the workdir, cleaned up afterwards.
    Worktree,
}
