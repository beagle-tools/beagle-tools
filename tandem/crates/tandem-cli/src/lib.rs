//! `tandem-cli` — read Tandem's local store from a script or an agent.
//!
//! Tandem keeps everything in one SQLite file with no server in front
//! of it, which makes "just read the file" the integration path. This
//! is that path with the paths, table names and JSON digging already
//! done, and it opens the database READ-ONLY: a tool that can only
//! read can't corrupt a review you're in the middle of. The one
//! exception is `review`, which appends a request to the review queue
//! (see `docs/CLI.md`).
//!
//! The entry point is [`run`], which takes the arguments after the
//! program (or tool) name and never reads `std::env::args` itself, so it
//! behaves the same standalone and linked into the `beagle` command.

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use rusqlite::{Connection, OpenFlags};

mod query;
#[cfg(test)]
mod query_tests;
mod queue;
mod update;

/// One line for `beagle help`.
pub const ABOUT: &str = "Read Tandem's local review data as JSON, and queue reviews";

#[derive(Parser)]
#[command(name = "tandem-cli", about = ABOUT, version)]
struct Cli {
    /// Database to read. Defaults to `$TANDEM_DB`, else the installed
    /// app's store.
    #[arg(long, global = true)]
    db: Option<PathBuf>,
    /// Read the `make dev` store instead of the installed app's.
    #[arg(long, global = true)]
    dev: bool,
    /// Indent the JSON.
    #[arg(long, global = true)]
    pretty: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Where the data lives: database, run logs, settings.
    Paths,
    /// Cached pull requests, newest first.
    Prs {
        /// Only this repo, as owner/name.
        #[arg(long)]
        repo: Option<String>,
        /// At most this many.
        #[arg(long, default_value_t = 50)]
        limit: u32,
    },
    /// Local review comments for one PR, oldest first.
    Comments {
        /// The repo, as owner/name.
        #[arg(long)]
        repo: String,
        /// The PR number.
        #[arg(long)]
        pr: u64,
        /// Nest replies under their root instead of a flat list.
        #[arg(long)]
        threads: bool,
    },
    /// The agent's PR summary, if one has been written.
    Summary {
        /// The repo, as owner/name.
        #[arg(long)]
        repo: String,
        /// The PR number.
        #[arg(long)]
        pr: u64,
    },
    /// Agent runs, newest first.
    Runs {
        /// Only this repo, as owner/name.
        #[arg(long)]
        repo: Option<String>,
        /// Only this PR (needs --repo).
        #[arg(long, requires = "repo")]
        pr: Option<u64>,
        /// At most this many.
        #[arg(long, default_value_t = 20)]
        limit: u32,
    },
    /// PRs waiting on your review that no agent has reviewed yet —
    /// the question a scheduled job asks.
    Queue {
        /// Include ones a review has already been queued for.
        #[arg(long)]
        all: bool,
    },
    /// Ask Tandem to review a PR. Tandem starts it with your agent
    /// config; this only records the request.
    Review {
        /// The repo, as owner/name.
        #[arg(long)]
        repo: String,
        /// The PR number.
        #[arg(long)]
        pr: u64,
        /// Which agent. Defaults to Tandem's first configured one.
        #[arg(long)]
        agent: Option<String>,
        /// Who asked, for the record.
        #[arg(long, default_value = "tandem-cli")]
        by: String,
    },
    /// Check for a newer Tandem, or install it.
    Update {
        /// Report versions and stop.
        #[arg(long)]
        check: bool,
    },
}

/// Run with the arguments AFTER the program name. Prints JSON on stdout;
/// errors go to stderr with exit code 1, usage errors with clap's code.
pub fn run(args: Vec<OsString>) -> ExitCode {
    let program = std::iter::once(OsString::from("tandem-cli"));
    let cli = match Cli::try_parse_from(program.chain(args)) {
        Ok(cli) => cli,
        Err(e) => {
            // --help and --version arrive here too, as "errors" that
            // print to stdout and exit 0.
            let code = e.exit_code();
            let _ = e.print();
            return ExitCode::from(u8::try_from(code).unwrap_or(2));
        }
    };
    match dispatch(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("tandem-cli: {message}");
            ExitCode::FAILURE
        }
    }
}

/// Mirrors the app's own directory choice, including the separate
/// `tandem-dev` store a `make dev` run uses.
fn default_db(dev: bool) -> Option<PathBuf> {
    let name = if dev { "tandem-dev" } else { "tandem" };
    Some(dirs::data_dir()?.join(name).join("cache.sqlite3"))
}

fn resolve_db(cli: &Cli) -> Result<PathBuf, String> {
    if let Some(path) = &cli.db {
        return Ok(path.clone());
    }
    if let Some(env) = std::env::var_os("TANDEM_DB").filter(|v| !v.is_empty()) {
        return Ok(PathBuf::from(env));
    }
    default_db(cli.dev).ok_or_else(|| "no data directory on this platform".to_owned())
}

fn dispatch(cli: &Cli) -> Result<(), String> {
    // Updating touches the network and this binary, never the store.
    if let Command::Update { check } = cli.command {
        let value = if check {
            update::check()?
        } else {
            update::perform()?
        };
        query::print(&value, cli.pretty);
        return Ok(());
    }

    let db = resolve_db(cli)?;
    if matches!(cli.command, Command::Paths) {
        query::print(&query::paths(&db), cli.pretty);
        return Ok(());
    }
    if !db.exists() {
        return Err(format!(
            "no database at {} — open Tandem once, or pass --db",
            db.display()
        ));
    }

    // The one command that writes: it records a request, it does not
    // touch review data.
    if let Command::Review {
        repo,
        pr,
        agent,
        by,
    } = &cli.command
    {
        let value = queue::request_review(&db, repo, *pr, agent.as_deref(), by)?;
        query::print(&value, cli.pretty);
        return Ok(());
    }

    // Read-only, so this never creates, migrates or writes. Tandem's WAL
    // mode means the app can stay open.
    let conn = Connection::open_with_flags(&db, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| format!("can't open {}: {e}", db.display()))?;
    let value = read(&conn, &cli.command)?;
    query::print(&value, cli.pretty);
    Ok(())
}

/// The read-only commands. Callers handle `paths`, `review` and `update`
/// before opening the store; they answer `null` here.
fn read(conn: &Connection, command: &Command) -> Result<serde_json::Value, String> {
    match command {
        Command::Prs { repo, limit } => query::prs(conn, repo.as_deref(), *limit),
        Command::Comments { repo, pr, threads } => query::comments(conn, repo, *pr, *threads),
        Command::Summary { repo, pr } => query::summary(conn, repo, *pr),
        Command::Runs { repo, pr, limit } => query::runs(conn, repo.as_deref(), *pr, *limit),
        Command::Queue { all } => queue::queue(conn, *all),
        Command::Paths | Command::Review { .. } | Command::Update { .. } => {
            Ok(serde_json::Value::Null)
        }
    }
}
