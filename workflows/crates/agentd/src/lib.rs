//! `agentd` — the daemon ("kubelet") of the agent orchestrator.
//!
//! Each tick it reconciles manifests into the store, schedules due cron runs,
//! spawns pending runs in tmux, and monitors running ones. Killing the daemon
//! is always safe: runs keep going in their tmux sessions, and a startup
//! recovery pass finalizes anything that finished while it was down.
//!
//! A library entry point ([`run_as`]): the unified `beagle` command runs it
//! as `beagle agent daemon run`, and `main.rs` is the standalone `agentd`.

mod caps;
mod ctlcmd;
mod gitguard;
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

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use agent_core::paths;
use agent_core::state::Store;
use anyhow::{Context, Result};
use chrono::Utc;
use clap::{CommandFactory, FromArgMatches, Parser};
use tracing::{error, info};

/// The standalone daemon's program name.
pub const STANDALONE: &str = "agentd";

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

/// Run the daemon as `program` (`agentd`, `agentctl daemon run` or
/// `beagle agent daemon run`) with the arguments after it. Returns only on
/// `--once`, `--help`/`--version`, or a fatal error.
pub fn run_as(program: &str, args: Vec<OsString>) -> ExitCode {
    let command_line = std::iter::once(OsString::from(program)).chain(args);
    let parsed = Cli::command()
        .name(program.to_owned())
        .bin_name(program.to_owned())
        .try_get_matches_from(command_line)
        .and_then(|m| Cli::from_arg_matches(&m));
    let cli = match parsed {
        Ok(cli) => cli,
        Err(e) => {
            let _ = e.print();
            return ExitCode::from(u8::try_from(e.exit_code()).unwrap_or(2));
        }
    };
    let exe = std::env::current_exe().ok();
    let sibling = exe
        .as_deref()
        .and_then(ctlcmd::sibling)
        .is_some_and(|p| p.is_file());
    ctlcmd::set(ctlcmd::command(program, exe.as_deref(), sibling));
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<()> {
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
    // The no-push-to-main guard. Runs still start without it, so say so
    // as an error: the GitHub branch protection is then the only guard.
    if let Err(e) = gitguard::install(&agent_core::guard::protected_orgs()) {
        tracing::error!(error = %e, "could not install the git push guard");
    }
    let mut store = Store::open_default().context("failed to open state store")?;
    let mut monitor = monitor::Monitor::new(notify_cfg.clone());
    info!(manifests = %manifests.display(), "agentd starting");
    // Best-effort first beat; a failure is warned about on every tick below.
    let _ = agent_core::daemon::beat(cli.tick_seconds);

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
        // The heartbeat is how agenttop and agentctl tell a stopped daemon
        // from an idle one (see agent_core::daemon).
        if let Err(e) = agent_core::daemon::beat(cli.tick_seconds) {
            tracing::warn!(error = %e, "could not write the heartbeat");
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
    retention::tick(
        store,
        &specs,
        Utc::now(),
        log_retention_hours,
        &retention::Dirs::real()?,
    )?;
    Ok(())
}

/// A stock icon: first the manifests repo's (resolve the usually symlinked
/// manifests dir and look under `../assets/notify/`), then the copy
/// `agentctl daemon install` unpacks into `<state>/assets/notify/` — a brew
/// install has no repo to look in.
fn default_icon(manifests: &Path, name: &str) -> Option<PathBuf> {
    let state = paths::state_dir().ok();
    stock_icon(manifests, state.as_deref(), name)
}

fn stock_icon(manifests: &Path, state_dir: Option<&Path>, name: &str) -> Option<PathBuf> {
    let from_repo = std::fs::canonicalize(manifests)
        .ok()
        .and_then(|m| m.parent().map(|repo| repo.join("assets/notify").join(name)));
    let from_state = state_dir.map(|s| s.join("assets/notify").join(name));
    [from_repo, from_state]
        .into_iter()
        .flatten()
        .find(|icon| icon.is_file())
}

#[cfg(test)]
// Tests should fail loudly on a broken fixture; unwrapping is the clearest way.
#[allow(clippy::unwrap_used)]
mod tests {
    use super::stock_icon;

    #[test]
    fn stock_icon_prefers_the_repo_then_the_state_dir() {
        let tmp = std::env::temp_dir().join(format!("agentd-icon-test-{}", std::process::id()));
        let manifests = tmp.join("repo/manifests");
        let state = tmp.join("state");
        std::fs::create_dir_all(&manifests).unwrap();
        std::fs::create_dir_all(state.join("assets/notify")).unwrap();
        std::fs::write(state.join("assets/notify/agentd.png"), b"x").unwrap();
        let found = stock_icon(&manifests, Some(&state), "agentd.png").unwrap();
        assert!(found.starts_with(&state));

        std::fs::create_dir_all(tmp.join("repo/assets/notify")).unwrap();
        std::fs::write(tmp.join("repo/assets/notify/agentd.png"), b"x").unwrap();
        let found = stock_icon(&manifests, Some(&state), "agentd.png").unwrap();
        assert!(found.ends_with("repo/assets/notify/agentd.png"));
        assert_eq!(stock_icon(&manifests, None, "missing.png"), None);
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
