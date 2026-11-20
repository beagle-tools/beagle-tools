//! Command dispatch: `beagle scout` opens the TUI; the subcommands give
//! agents (and scripts) a typed way to create and update projects and tasks.
//!
//! Parsing lives in [`crate::cli`] and is pure; this module does the I/O.
//! The store root is chosen by [`root::resolve`].

// Command output goes through these instead of std's `println!`/`print!`,
// which panic when stdout is a closed pipe (`beagle scout list | head -1`).
// A reader that stops reading is not an error worth crashing over, so the
// write result is deliberately dropped.
macro_rules! println {
    ($($arg:tt)*) => {{
        use std::io::Write as _;
        let _ = writeln!(std::io::stdout(), $($arg)*);
    }};
}
macro_rules! print {
    ($($arg:tt)*) => {{
        use std::io::Write as _;
        let _ = write!(std::io::stdout(), $($arg)*);
    }};
}

mod hook_cmd;
mod listing;
mod root;
mod session_cmd;
mod skill_cmd;
mod status;

use std::env;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;

use crate::cli::{self, Command, USAGE};
use crate::model::Slug;
use crate::store::Store;
use crate::{Error, Result};

/// Parses `args` (everything after `scout`) and runs the command.
pub(crate) fn main(args: Vec<OsString>) -> ExitCode {
    let mut strings = Vec::with_capacity(args.len());
    for arg in args {
        match arg.into_string() {
            Ok(s) => strings.push(s),
            Err(bad) => {
                eprintln!(
                    "beagle scout: argument {} is not valid UTF-8",
                    bad.display()
                );
                return ExitCode::from(2);
            }
        }
    }
    let invocation = match cli::parse_args(strings) {
        Ok(invocation) => invocation,
        Err(message) => {
            eprintln!("beagle scout: {message}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match dispatch(invocation) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("beagle scout: {e}");
            ExitCode::FAILURE
        }
    }
}

fn dispatch(invocation: cli::Invocation) -> Result<()> {
    let root = invocation.root;
    let store = || resolve_root(root.clone()).map(Store::new);
    match invocation.command {
        Command::Help => print!("{USAGE}"),
        Command::Version => println!("beagle scout {}", env!("CARGO_PKG_VERSION")),
        Command::Init(dir) => root::init_command(dir)?,
        Command::Root => root::print_current(root)?,
        Command::SkillInstall => skill_cmd::install()?,
        Command::SkillStatus => skill_cmd::status()?,
        Command::Hook => hook_cmd::record(root),
        Command::HookInstall => hook_cmd::install()?,
        Command::HookUninstall => hook_cmd::uninstall()?,
        Command::HookStatus => hook_cmd::status()?,
        Command::WatchList => crate::watch::list(&resolve_root(root)?),
        Command::Watch(args) => session_cmd::watch(&store()?, args)?,
        Command::Tui => listing::tui(store()?)?,
        Command::List { json } => listing::list(&store()?, json)?,
        Command::Show { project } => listing::show(&store()?, &project)?,
        Command::Path { project, task } => listing::print_path(&store()?, project, task)?,
        Command::New {
            project,
            title,
            links,
        } => {
            let p = store()?.create_project(&Slug::parse(&project)?, &title, links)?;
            println!("created project {} (planning)", p.id);
            println!(
                "  write the goal, problem and stack tour in: {}",
                p.file().display()
            );
            println!(
                "  then add tasks: beagle scout task add {} <slug> --title \"...\"",
                p.id
            );
        }
        Command::ProjectStatus { project, status } => {
            status::set_project(&store()?, &project, status)?;
        }
        Command::Link {
            project,
            task,
            link,
        } => {
            store()?.add_link(&project, task.as_deref(), &link)?;
            println!("linked {link}");
        }
        Command::TaskAdd {
            project,
            slug,
            title,
            links,
        } => {
            let task = store()?.add_task(&project, &Slug::parse(&slug)?, &title, links)?;
            println!("added task {}", task.id);
            println!("  fill in: {}", task.path.display());
        }
        Command::TaskStatus {
            project,
            task,
            status,
            reason,
        } => status::set_task(&store()?, &project, &task, status, reason.as_deref())?,
        Command::TaskAttach {
            project,
            task,
            session,
        } => session_cmd::attach(&store()?, &project, &task, session)?,
        Command::SessionShow {
            project,
            task,
            follow,
        } => session_cmd::show(&store()?, &project, &task, follow)?,
        Command::NotifyTest => status::notify_test(),
        Command::NotifySetup => status::notify_setup()?,
        Command::TaskRm { project, task } => {
            let task = store()?.remove_task(&project, &task)?;
            println!("removed task {}", task.id);
        }
        Command::Log {
            project,
            task,
            message,
        } => {
            store()?.append_log(&project, task.as_deref(), &message)?;
        }
    }
    Ok(())
}

/// The store `beagle scout` uses from this process, with no `--root`.
pub(crate) fn default_root() -> Result<PathBuf> {
    resolve_root(None)
}

/// The store this command uses; see [`root::resolve`] for the order.
fn resolve_root(explicit: Option<PathBuf>) -> Result<PathBuf> {
    root::current(explicit).map(|(path, _)| path)
}

fn home() -> Result<PathBuf> {
    beagle_utils::paths::home_dir().ok_or(Error::NoHome)
}

/// `$CLAUDE_CODE_SESSION_ID`, when set and non-empty.
fn claude_session() -> Option<String> {
    env::var("CLAUDE_CODE_SESSION_ID")
        .ok()
        .filter(|s| !s.is_empty())
}

/// The first eight characters of a session id, for display.
fn short(id: &str) -> &str {
    id.get(..8).unwrap_or(id)
}
