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
    /// Run a failed, timed-out or killed step again, in its chain: same
    /// parent, params and model, so it reads the same handover and the rest
    /// of the chain carries on from there. Works for a week after the step.
    Resume {
        /// The run id of the step that failed.
        run_id: String,
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
    /// The live dashboard (jobs → runs → logs): watch, run, resume, attach,
    /// kill. Everything after `top` goes to it.
    #[command(disable_help_flag = true)]
    Top {
        /// The dashboard's own flags (`--help`, `--version`).
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<std::ffi::OsString>,
    },
    /// What broke — failed or timed-out runs, and ERROR lines in runs that
    /// carried on — grouped so a repeated failure is one entry with a
    /// count. Never looks back more than 48 hours.
    Errors {
        /// Start of the window, RFC 3339.
        #[arg(long, value_name = "TIME", conflicts_with = "since_last")]
        since: Option<String>,
        /// Start at this agent's previous successful run (excluding the
        /// current run, `$AGENTD_RUN_ID`), as `<project>/<agent>`.
        #[arg(long, value_name = "PROJECT/AGENT")]
        since_last: Option<String>,
        /// How far back at most, in hours (capped at 48).
        #[arg(long, default_value_t = 48)]
        max_hours: u32,
        /// Print JSON instead of markdown.
        #[arg(long)]
        json: bool,
        /// Write errors.md and errors.json into DIR and print only the
        /// number of distinct errors.
        #[arg(long, value_name = "DIR")]
        write: Option<PathBuf>,
    },
    /// The daemon: run it, or manage it under launchd (macOS) — install,
    /// restart, or remove it.
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },
}

/// `daemon` actions.
#[derive(Debug, Subcommand)]
pub enum DaemonAction {
    /// Write ~/Library/LaunchAgents/com.agentd.daemon.plist for the daemon
    /// from this same install, unpack the notification icons, build the
    /// branded notifiers, and (re)load the daemon.
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
    /// Run the daemon itself in the foreground — what the launchd unit
    /// starts. Everything after `run` goes to the daemon (`run --help`
    /// lists its flags).
    #[command(disable_help_flag = true)]
    Run {
        /// The daemon's own flags, e.g. `--once` or `--tick-seconds 10`.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<std::ffi::OsString>,
    },
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
