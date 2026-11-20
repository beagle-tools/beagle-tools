//! `tandem-tui`: a terminal review queue on Tandem's store.
//!
//! You queue PRs (in the TUI, or `beagle tandem add` from anywhere); a
//! reviewer — typically an agentd job — claims each one, an agent reviews
//! it into local comments, and you go through them in the TUI — diff,
//! summary, comments — posting the ones you pick and approving, both only
//! on a confirmed keypress.
//!
//! This crate is a library linked by `tandem-cli`: with no subcommand,
//! `tandem-cli` / `beagle tandem` opens the TUI ([`open`]), and [`Command`]
//! is flattened into its subcommands ([`run`]).

mod config;
mod context;
mod gh;
mod ingest;
mod jobs;
mod pr_ref;
pub mod read;
mod state;
mod ui;
mod verdict;

use std::path::PathBuf;
use std::process::{ExitCode, Stdio};

use clap::Subcommand;
use tandem_cache::{Cache, ReviewTriage};
use tandem_core::{Result, TandemError};

use crate::config::Config;
use crate::pr_ref::PrRef;

/// The review queue's subcommands. The host CLI flattens these into its own.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Queue PRs for the review TUI (URL or owner/repo#n), then start the
    /// reviewer with the config's `kick` command.
    Add {
        /// The PRs, as `https://github.com/o/r/pull/n` or `o/r#n`.
        #[arg(required = true)]
        prs: Vec<String>,
        /// Who asked, shown in the queue.
        #[arg(long, default_value = "cli")]
        by: String,
        /// Only queue; don't run the config's `kick` command.
        #[arg(long)]
        no_kick: bool,
    },
    /// For the reviewer: claim the oldest queued PR and write its review
    /// context to `<out>/context.md`. Prints `<run id> <context path>`, or
    /// nothing when the queue is empty.
    Claim {
        /// Directory to write `context.md` into (created if missing).
        #[arg(long)]
        out: PathBuf,
        /// The agent name recorded on the run.
        #[arg(long, default_value = "claude")]
        agent: String,
    },
    /// For the reviewer: load an agent's review (JSON lines) into a
    /// claimed run. Loads nothing, and says what to fix, if any line is
    /// wrong.
    Ingest {
        /// The run id `claim` printed.
        #[arg(long)]
        run: String,
        /// The JSON-lines file the agent wrote.
        #[arg(long)]
        file: PathBuf,
    },
    /// For the reviewer: mark a claimed run failed.
    Fail {
        /// The run id `claim` printed.
        #[arg(long)]
        run: String,
        /// Why, shown in the TUI.
        #[arg(long)]
        error: String,
    },
    /// Every PR in the review TUI's queue and where it stands.
    List,
}

/// Opens the review TUI on its store. Returns when you quit.
pub fn open() -> Result<()> {
    let db = config::db_path()?;
    let config = Config::load()?;
    let cache = Cache::open(&db)?;
    ui::run(cache, db, config)
}

/// Runs one queue subcommand. `program` is how the reviewer is told to
/// call back (`beagle tandem`, `tandem-cli`), in the context `claim` writes.
pub fn run(command: Command, program: &str) -> Result<ExitCode> {
    let db = config::db_path()?;
    let config = Config::load()?;
    let cache = Cache::open(&db)?;
    match command {
        Command::Add { prs, by, no_kick } => {
            for raw in &prs {
                let pr = PrRef::parse(raw)
                    .ok_or_else(|| TandemError::Config(format!("not a PR: {raw}")))?;
                jobs::add(&cache, &pr, &by)?;
                println!("queued {}", pr.label());
            }
            if !no_kick {
                if let Some(warning) = kick(&config) {
                    eprintln!("{warning}");
                }
            }
            let last_claim = cache
                .list_review_jobs()?
                .into_iter()
                .filter_map(|j| j.started_at)
                .max();
            if let Some(warning) = idle_warning(
                !config.kick.is_empty(),
                last_claim,
                chrono::Utc::now(),
                program,
            ) {
                eprintln!("{warning}");
            }
        }
        Command::Claim { out, agent } => {
            let claimed = block_on(async {
                let client = gh::connect(&config).await?;
                jobs::claim(&cache, &client, &out, &agent, program).await
            })??;
            if let Some(c) = claimed {
                println!("{} {}", c.run_id, c.context.display());
                eprintln!("claimed {}", c.pr.label());
            }
        }
        Command::Ingest { run, file } => return ingest(&cache, &run, &file, program),
        Command::Fail { run, error } => jobs::fail(&cache, &run, &error)?,
        Command::List => {
            for row in state::rows(&cache)? {
                let verdict = row.verdict.map_or("", verdict::Verdict::badge);
                println!(
                    "{:<9} {:<7} {:<40} {}",
                    row.stage.label(),
                    verdict,
                    row.pr.label(),
                    row.title.unwrap_or_default()
                );
            }
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn ingest(cache: &Cache, run: &str, file: &std::path::Path, program: &str) -> Result<ExitCode> {
    let text = std::fs::read_to_string(file)
        .map_err(|e| TandemError::Config(format!("can't read {}: {e}", file.display())))?;
    Ok(match jobs::ingest(cache, run, &text) {
        Ok(n) => {
            println!("loaded {n} comment(s) for run {run}");
            ExitCode::SUCCESS
        }
        Err(jobs::IngestError::Review(errors)) => {
            eprintln!("nothing loaded — fix these and run ingest again:");
            for e in errors {
                eprintln!("  {e}");
            }
            ExitCode::FAILURE
        }
        Err(jobs::IngestError::Run(e)) => {
            eprintln!("{program}: {e}");
            ExitCode::FAILURE
        }
        Err(jobs::IngestError::Store(e)) => return Err(e),
    })
}

/// Where the review TUI keeps its data, as JSON for `paths`: the store,
/// the config file and the GitHub account it acts as.
pub fn paths() -> serde_json::Value {
    let account = match Config::load() {
        Ok(config) => serde_json::json!(config.github_user),
        Err(e) => serde_json::json!({ "error": e.to_string() }),
    };
    serde_json::json!({
        // No data directory is reported as a null store, not an error.
        "store": config::db_path().ok(),
        "config": config::config_path(),
        "github_user": account,
    })
}

fn block_on<F: std::future::Future>(future: F) -> Result<F::Output> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    Ok(rt.block_on(future))
}

/// How recently a reviewer must have claimed a PR for `add` to trust that
/// one is running.
const REVIEWER_ACTIVE_HOURS: i64 = 24;

/// The warning `add` prints when nothing looks set to work the queue: no
/// `kick` configured and no claim in [`REVIEWER_ACTIVE_HOURS`]. `None` when a
/// kick is configured or a reviewer claimed something recently.
pub(crate) fn idle_warning(
    kick_configured: bool,
    last_claim: Option<chrono::DateTime<chrono::Utc>>,
    now: chrono::DateTime<chrono::Utc>,
    program: &str,
) -> Option<String> {
    if kick_configured
        || last_claim.is_some_and(|t| now - t < chrono::Duration::hours(REVIEWER_ACTIVE_HOURS))
    {
        return None;
    }
    let config = config::config_path().map_or_else(
        || "the review TUI config".to_owned(),
        |p| p.display().to_string(),
    );
    Some(format!(
        "warning: nothing may review this — no `kick` command is set in {config} and no \
         reviewer has claimed a PR in the last 24 hours. Set `kick:` (e.g. an agentd job), \
         or run a reviewer that calls `{program} claim`. (`{program} app-review` queues for \
         the Tandem desktop app instead.)"
    ))
}

/// Starts the reviewer now (the config's `kick`), detached; best effort —
/// a scheduled reviewer picks the PR up anyway. Returns a warning when it
/// could not start.
pub(crate) fn kick(config: &Config) -> Option<String> {
    let (program, args) = config.kick.split_first()?;
    let spawned = beagle_utils::process::command(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    match spawned {
        // Detached on purpose: the reviewer outlives this command, and
        // dropping the handle neither waits for nor kills it.
        Ok(_child) => None,
        Err(e) => Some(format!("could not start the reviewer ({program}): {e}")),
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
