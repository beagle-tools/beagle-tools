//! Command-line interface types (clap derive).

use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// kubectl-style CLI for the agentd stack.
#[derive(Debug, Parser)]
#[command(
    name = "agentctl",
    version,
    about = "Inspect and control AI coding agents"
)]
pub struct Cli {
    /// Job to run: a bare agent name, or `<project>/<agent>` to disambiguate.
    /// The job's declared `params:` are enforced before anything is queued.
    #[arg(short = 'j', long, value_name = "JOB")]
    pub job: Option<String>,
    /// KEY=VALUE parameters for `-j`. Comma-separate several
    /// (`-p url=...,depth=2`) and/or repeat the flag. A value containing a
    /// comma needs its own `-p`.
    #[arg(short = 'p', long, value_name = "K=V,...")]
    pub param: Vec<String>,
    /// Model for a `-j` run, overriding the manifest's `model:`.
    #[arg(short = 'm', long, value_name = "MODEL")]
    pub model: Option<String>,
    #[command(subcommand)]
    pub command: Option<Command>,
}

/// Top-level subcommands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Load manifests and sync the declared agent set into the store.
    Apply {
        /// Manifests root directory (default: ~/.config/agentd/manifests).
        #[arg(long, value_name = "DIR")]
        manifests: Option<PathBuf>,
    },
    /// List resources.
    Get {
        #[command(subcommand)]
        resource: GetResource,
    },
    /// Trigger a manual run of an agent.
    Run {
        /// Agent to run, as `<project>/<agent>`.
        target: String,
        /// Run id that triggered this run — records DAG lineage and exposes
        /// the parent's data dir as `$AGENTD_PARENT_DATA_DIR`.
        #[arg(long, value_name = "RUN_ID")]
        parent: Option<String>,
        /// Per-run parameter, KEY=VALUE (repeatable) — exported to the run
        /// as `AGENTD_PARAM_KEY`.
        #[arg(long, value_name = "KEY=VALUE")]
        param: Vec<String>,
        /// Model for this run only, overriding the manifest's `model:`
        /// (e.g. opus, sonnet, haiku).
        #[arg(long, value_name = "MODEL")]
        model: Option<String>,
    },
    /// Print (or follow) the event log of a run.
    Logs {
        /// A run id, or `<project>/<agent>` for that agent's latest run.
        target: String,
        /// Follow the run until it reaches a terminal state.
        #[arg(short, long)]
        follow: bool,
        /// Only show events of this kind (e.g. state, `tool_use`, message, raw).
        #[arg(long, value_name = "K")]
        kind: Option<String>,
    },
    /// Kill a running run's tmux session and mark the run killed.
    Kill {
        /// The run id to kill.
        run_id: String,
    },
    /// Attach to a running run's tmux session (replaces this process).
    Attach {
        /// The run id to attach to.
        run_id: String,
    },
    /// One-screen summary of agents and runs.
    Status,
    /// Run agentd under launchd (macOS): install, restart, or remove it.
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },
}

/// `agentctl daemon` actions.
#[derive(Debug, Subcommand)]
pub enum DaemonAction {
    /// Write ~/Library/LaunchAgents/com.agentd.daemon.plist for the agentd
    /// installed next to this agentctl, unpack the notification icons, build
    /// the branded notifiers, and (re)load the daemon.
    Install {
        /// Only print the rendered launchd unit; change nothing.
        #[arg(long)]
        print: bool,
    },
    /// Restart the daemon — run after every upgrade so the new agentd
    /// reconciles manifests.
    Restart,
    /// Stop the daemon and remove its launchd unit (state and logs stay).
    Uninstall,
}

/// Resources listable via `agentctl get`.
#[derive(Debug, Subcommand)]
pub enum GetResource {
    /// Projects with their agent counts.
    Projects,
    /// Runnable jobs and the parameters each declares.
    Jobs,
    /// Declared agents.
    Agents {
        /// Filter by project.
        #[arg(short, long)]
        project: Option<String>,
    },
    /// Runs, newest first.
    Runs {
        /// Filter by project.
        #[arg(short, long)]
        project: Option<String>,
        /// Filter by agent name.
        #[arg(short, long)]
        agent: Option<String>,
        /// Filter by run state (pending|running|succeeded|failed|timedout|killed).
        #[arg(long, value_name = "STATE")]
        state: Option<String>,
        /// Maximum number of rows.
        #[arg(short = 'n', long, default_value_t = 20)]
        limit: u32,
    },
}
