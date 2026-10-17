//! CLI entry point: `beagle` opens the TUI; the subcommands give scripts
//! (and Claude) a typed way to create, inspect, and update workspaces — and
//! the binary itself (`beagle update`).
//!
//! Argument parsing lives in [`cli`] and is pure (no filesystem, no
//! network); `run` does the I/O. `--root` resolution consults the config
//! file: explicit flag → config `root` → current directory.

mod cli;
mod commands;

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use beagle::store::{new_meta, Store};
use beagle::{config, ui, update, Error};

use cli::{Command, USAGE};
use commands::{
    install_version, latest_release, run_agent_service, run_archive, run_config, run_context,
    run_handoff, run_init, run_list, run_pr_add, run_pr_list, run_set_published, run_set_status,
    run_similar, run_skill, run_ticket_add, run_ticket_list, run_ticket_sync, run_unarchive,
    run_version_list,
};

fn main() -> ExitCode {
    let command = match cli::parse_args(env::args().skip(1)) {
        Ok(command) => command,
        Err(message) => {
            eprintln!("error: {message}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match run(command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(command: Command) -> Result<(), Error> {
    match command {
        Command::Help => {
            println!("{USAGE}");
            Ok(())
        }
        Command::Version => {
            println!(
                "beagle {} ({})",
                update::Version::current(),
                update::release_target().unwrap_or("source build"),
            );
            Ok(())
        }
        Command::Tui { root } => {
            let config = effective_config().ok().flatten();
            let notify = config.as_ref().and_then(|c| c.notify).unwrap_or(false);
            // Absent `[notify_events]` means every event fires.
            let events = config
                .and_then(|c| c.notify_events)
                .unwrap_or_else(config::NotifyEvents::all);
            ui::run(Store::open(&effective_root(root)?)?, notify, events)
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
        Command::Update { version } => {
            let version = match version {
                Some(version) => version,
                None => latest_release()?,
            };
            install_version(version)
        }
        Command::VersionList => run_version_list(),
        Command::Banner => {
            println!(
                "{}\nbeagle {} — a TUI for root-cause analysis workspaces",
                beagle::banner::BANNER,
                update::Version::current(),
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
/// would quietly open the wrong root.
fn effective_root(explicit: Option<PathBuf>) -> Result<PathBuf, Error> {
    if let Some(root) = explicit {
        return Ok(root);
    }
    if let Some(config) = effective_config()? {
        if let Some(root) = config.root {
            return Ok(root);
        }
    }
    let cwd = env::current_dir().map_err(|e| Error::io(".", e))?;
    Ok(beagle::store::discover_root(&cwd))
}

/// The config governing this invocation: the nearest `.beagle` walking up
/// from the working directory (git-style), falling back to the global
/// config file — see [`config::load_effective`].
fn effective_config() -> Result<Option<config::Config>, Error> {
    let cwd = env::current_dir().map_err(|e| Error::io(".", e))?;
    config::load_effective(&cwd)
}
