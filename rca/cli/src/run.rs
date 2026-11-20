//! The CLI entry points: `beagle rca` opens the TUI; the subcommands give
//! scripts (and agents) a typed way to create, inspect and update
//! workspaces.
//!
//! [`run`] is the tool contract the unified `beagle` command links
//! (`beagle rca …`); [`run_standalone`] backs the standalone rca `beagle`
//! binary, which additionally updates itself. Argument parsing lives in
//! `crate::cli` and is pure (no filesystem, no network); this module does
//! the I/O. `--root` resolution consults the config file: explicit flag →
//! config `root` → current directory.

use std::env;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;

use crate::cli::{self, Command, USAGE};
use crate::commands::{
    install_version, latest_release, run_agent_service, run_archive, run_config, run_context,
    run_handoff, run_init, run_list, run_pr_add, run_pr_list, run_set_published, run_set_status,
    run_similar, run_skill, run_ticket_add, run_ticket_list, run_ticket_sync, run_unarchive,
    run_version_list,
};
use crate::store::{new_meta, Store};
use crate::{config, ui, update, Error};

/// One line for `beagle help`.
pub const ABOUT: &str = "root-cause analysis workspaces: a live TUI plus commands for agents";

/// Which binary is running rca, which decides who owns updating it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Linked into the unified `beagle` command as `beagle rca`: updating
    /// is `beagle update`'s job, since an rca-only download would replace
    /// every other tool in the binary.
    Unified,
    /// The standalone rca `beagle` binary, which updates itself.
    Standalone,
}

impl Mode {
    /// What error messages start with: the command the user typed.
    const fn prefix(self) -> &'static str {
        match self {
            Self::Unified => "beagle rca",
            Self::Standalone => "beagle",
        }
    }

    /// `command`, unless this binary has no such command: under `beagle`,
    /// `update` and `version list` belong to `beagle` itself (an rca-only
    /// download would replace every other tool), so they are usage errors.
    fn allows(self, command: Command) -> Result<Command, String> {
        match (self, &command) {
            (Self::Unified, Command::Update { .. }) => {
                Err("unknown command `update` — `beagle update` updates every tool".to_owned())
            }
            (Self::Unified, Command::VersionList) => Err(
                "unknown command `version list` — `beagle version list` lists releases".to_owned(),
            ),
            _ => Ok(command),
        }
    }
}

/// Runs rca with the arguments AFTER `beagle rca` (the unified `beagle`
/// command's tool contract). `--help` works; `update` and `version list`
/// are `beagle`'s own commands, so here they are usage errors.
pub fn run(args: Vec<OsString>) -> ExitCode {
    main_with(args, Mode::Unified)
}

/// Runs the standalone rca `beagle` binary with the arguments after the
/// program name. A leading `rca` is dropped, so the `beagle rca …` spelling
/// in the help, hints and skills works with this binary too.
pub fn run_standalone(args: Vec<OsString>) -> ExitCode {
    main_with(strip_rca(args), Mode::Standalone)
}

/// Drops one leading `rca`, the unified spelling (`beagle rca list`), so it
/// means the same as `beagle list` to the standalone binary. No RCA
/// subcommand is called `rca`, so nothing else can be meant by it.
fn strip_rca(mut args: Vec<OsString>) -> Vec<OsString> {
    if args.first().is_some_and(|a| a == "rca") {
        args.remove(0);
    }
    args
}

fn main_with(args: Vec<OsString>, mode: Mode) -> ExitCode {
    let args: Result<Vec<String>, _> = args.into_iter().map(OsString::into_string).collect();
    let parsed = match args {
        Ok(args) => cli::parse_args(args.into_iter()),
        Err(bad) => Err(format!(
            "argument `{}` is not valid UTF-8",
            bad.to_string_lossy()
        )),
    };
    let prefix = mode.prefix();
    let command = match parsed.and_then(|c| mode.allows(c)) {
        Ok(command) => command,
        Err(message) => {
            eprintln!("{prefix}: {message}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match execute(command, mode) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{prefix}: {e}");
            ExitCode::FAILURE
        }
    }
}

fn print_version(mode: Mode) {
    match mode {
        Mode::Unified => println!("beagle rca {}", update::current()),
        Mode::Standalone => println!(
            "beagle {} ({})",
            update::current(),
            update::release_target().unwrap_or("source build"),
        ),
    }
}

fn execute(command: Command, mode: Mode) -> Result<(), Error> {
    match command {
        Command::Help => {
            println!("{USAGE}");
            Ok(())
        }
        Command::Version => {
            print_version(mode);
            Ok(())
        }
        Command::Tui { root } => {
            let config = effective_config().ok().flatten();
            ui::run(
                Store::open(&effective_root(root)?)?,
                ui::Settings::from_config(config.as_ref()),
            )
        }
        Command::New {
            root,
            id,
            title,
            severity,
            systems,
        } => {
            let store = Store::open(&effective_root(root)?)?;
            let mut meta = new_meta(title, severity);
            meta.systems = systems;
            let dir = store.scaffold(&id, &meta)?;
            println!("created {}", dir.display());
            Ok(())
        }
        Command::Export { root, id, out } => {
            let store = Store::open(&effective_root(root)?)?;
            let path = store.export_to(&id, out.as_deref())?;
            println!("{}", path.display());
            Ok(())
        }
        Command::Context { root, id } => run_context(root, &id),
        Command::List {
            root,
            status,
            severity,
            archived,
            json,
        } => run_list(root, status, severity, archived, json),
        Command::Archive { root, id, force } => run_archive(root, &id, force),
        Command::Unarchive { root, id } => run_unarchive(root, &id),
        Command::SetPublished {
            root,
            id,
            published,
        } => run_set_published(root, &id, published),
        Command::Handoff { root, id } => run_handoff(root, &id),
        Command::SetStatus { root, id, status } => run_set_status(root, &id, status),
        Command::Log { root, id, message } => {
            let store = Store::open(&effective_root(root)?)?;
            let path = store.append_log(&id, &message)?;
            println!("logged to {}", path.display());
            Ok(())
        }
        Command::PrAdd { root, id, url } => run_pr_add(root, &id, &url),
        Command::PrList { root, id } => run_pr_list(root, &id),
        Command::TicketAdd {
            root,
            id,
            reference,
        } => run_ticket_add(root, &id, &reference),
        Command::TicketList { root, id } => run_ticket_list(root, &id),
        Command::TicketSync { root, id } => run_ticket_sync(root, id.as_ref()),
        Command::Similar { root, id } => run_similar(root, &id),
        Command::Init { root } => run_init(root),
        Command::Config => run_config(),
        Command::Skill { action } => run_skill(action),
        Command::Agent { action } => run_agent_service(action),
        // Under `beagle`, `Mode::allows` already turned these away.
        Command::Update { version } => install_version(match version {
            Some(version) => version,
            None => latest_release()?,
        }),
        Command::VersionList => run_version_list(),
        Command::Banner => {
            println!(
                "{}\nbeagle {} — a TUI for root-cause analysis workspaces",
                crate::banner::BANNER,
                update::current(),
            );
            Ok(())
        }
    }
}

/// Resolves the workspace root: explicit `--root` → the effective config's
/// `root` (a `.beagle` found walking up from the working directory, else
/// the global config) → nearest ancestor of the working directory that
/// already contains `rcas/` → the working directory. An invalid config
/// surfaces here rather than being silently ignored — otherwise beagle
/// would quietly open the wrong root. `beagle serve` uses this too, so the
/// phone reads the store `beagle rca` would.
///
/// # Errors
/// [`Error::Io`] when the working directory is unreadable, or the config
/// file's own load error.
pub fn effective_root(explicit: Option<PathBuf>) -> Result<PathBuf, Error> {
    if let Some(root) = explicit {
        return Ok(root);
    }
    if let Some(config) = effective_config()? {
        if let Some(root) = config.root {
            return Ok(root);
        }
    }
    let cwd = env::current_dir().map_err(|e| Error::io(".", e))?;
    Ok(crate::store::discover_root(&cwd))
}

/// The config governing this invocation: the nearest `.beagle` walking up
/// from the working directory (git-style), falling back to the global
/// config file — see [`config::load_effective`].
pub(crate) fn effective_config() -> Result<Option<config::Config>, Error> {
    let cwd = env::current_dir().map_err(|e| Error::io(".", e))?;
    config::load_effective(&cwd)
}

#[cfg(test)]
#[path = "tests/run.rs"]
mod tests;
