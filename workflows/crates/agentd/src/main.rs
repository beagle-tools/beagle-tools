//! `agentd` — the daemon ("kubelet") of the agent orchestrator.
//!
//! Each tick it reconciles manifests into the store, schedules due cron runs,
//! spawns pending runs in tmux, and monitors running ones. Killing the daemon
//! is always safe: runs keep going in their tmux sessions, and a startup
//! recovery pass finalizes anything that finished while it was down.

mod loglib;
mod monitor;
mod notify;
mod payload;
mod reconciler;
mod retention;
mod runenv;
mod runscript;
mod scheduler;
mod spawner;
mod summaries;
mod tmux;
mod usage;

use std::path::{Path, PathBuf};

use agent_core::paths;
use agent_core::state::Store;
use anyhow::{Context, Result};
use chrono::Utc;
use clap::Parser;
use tracing::{error, info};

/// Daemon that reconciles agent manifests, schedules cron runs, and
/// supervises tmux-hosted runs.
#[derive(Debug, Parser)]
#[command(name = "agentd", version)]
struct Cli {
    /// Manifests root directory (defaults to the standard location).
    #[arg(long)]
    manifests: Option<PathBuf>,

    /// Seconds to sleep between control-loop ticks.
    #[arg(long, default_value_t = 5)]
    tick_seconds: u64,

    /// Run exactly one tick, then exit.
    #[arg(long)]
    once: bool,

    /// Hours to keep a finished run's log file + event feed before pruning
    /// them (the run row itself is kept as history). 0 = keep forever.
    #[arg(long, default_value_t = 4)]
    log_retention_hours: u64,

    /// Default macOS-notification policy for finished runs: all, failures,
    /// or off. Per-agent `notify:` in the manifest overrides this.
    #[arg(long, default_value = "all")]
    notify: String,

    /// Default notification image. Falls back to `assets/notify/agentd.png`
    /// next to the manifests repo; per-agent `notify_icon:` overrides.
    #[arg(long)]
    notify_icon: Option<PathBuf>,

    /// Default image for failed runs (fallback: `assets/notify/agentd-fail.png`,
    /// then the regular icon). Per-agent `notify_icon_failure:` overrides.
    #[arg(long)]
    notify_icon_failure: Option<PathBuf>,

    /// macOS system sound for successful runs (Tink, Pop, Hero, Glass, …).
    #[arg(long, default_value = "Tink")]
    notify_sound: String,

    /// macOS system sound for failed/timed-out/killed runs.
    #[arg(long, default_value = "Funk")]
    notify_sound_failure: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    beagle_utils::log::init("info");

    let manifests = match cli.manifests {
        Some(dir) => dir,
        None => paths::manifests_dir().context("failed to resolve manifests dir")?,
    };
    let notify_cfg = notify::NotifyConfig {
        policy: cli
            .notify
            .parse()
            .context("--notify must be all, failures, or off")?,
        icon: cli
            .notify_icon
            .or_else(|| default_icon(&manifests, "agentd.png")),
        icon_failure: cli
            .notify_icon_failure
            .or_else(|| default_icon(&manifests, "agentd-fail.png")),
        sound: cli.notify_sound,
        sound_failure: cli.notify_sound_failure,
    };
    paths::ensure_state_dirs().context("failed to create state dirs")?;
    // A missing logger is worth a warning, not a refusal to start: script
    // stages that do not source it are unaffected, and one that does will
    // fail loudly on its own `. "$AGENTD_LOG_SH"`.
    if let Err(e) = loglib::install() {
        tracing::warn!(error = %e, "could not install the script logger");
    }
    let mut store = Store::open_default().context("failed to open state store")?;
    let mut monitor = monitor::Monitor::new(notify_cfg.clone());
    info!(manifests = %manifests.display(), "agentd starting");

    // Recovery: finalize runs whose tmux session died while the daemon was down.
    if let Err(e) = monitor.tick(&store) {
        error!(error = %format!("{e:#}"), "startup recovery pass failed");
    }

    loop {
        let result = run_tick(
            &mut store,
            &mut monitor,
            &manifests,
            cli.log_retention_hours,
            &notify_cfg,
        );
        if cli.once {
            return result;
        }
        if let Err(e) = result {
            error!(error = %format!("{e:#}"), "tick failed");
        }
        std::thread::sleep(std::time::Duration::from_secs(cli.tick_seconds));
    }
}

/// One pass of the control loop: reconcile → schedule → spawn → monitor →
/// prune expired logs.
fn run_tick(
    store: &mut Store,
    monitor: &mut monitor::Monitor,
    manifests: &Path,
    log_retention_hours: u64,
    notify_cfg: &notify::NotifyConfig,
) -> Result<()> {
    let specs = reconciler::reconcile(store, manifests)?;
    scheduler::tick(store, &specs, Utc::now())?;
    spawner::spawn_pending(store, notify_cfg)?;
    monitor.tick(store)?;
    retention::tick(store, &specs, Utc::now(), log_retention_hours)?;
    Ok(())
}

/// A stock icon shipped in the manifests repo: resolve the (usually
/// symlinked) manifests dir and look under `../assets/notify/`.
fn default_icon(manifests: &Path, name: &str) -> Option<PathBuf> {
    let repo = std::fs::canonicalize(manifests)
        .ok()?
        .parent()?
        .to_path_buf();
    let icon = repo.join("assets/notify").join(name);
    icon.is_file().then_some(icon)
}
