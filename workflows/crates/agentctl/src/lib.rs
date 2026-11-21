//! `agentctl` — the kubectl of the agentd stack.
//!
//! Reads and writes the shared `SQLite` store (and shells out to tmux for
//! kill/attach); never talks to the daemon directly — except `daemon run`,
//! which *is* the daemon, run in the foreground.
//!
//! A library entry point ([`run_as`]) so the unified `beagle` command can
//! link it as `beagle agent …`; `main.rs` is the standalone `agentctl`.

mod apply;
mod cli;
mod daemon;
mod errors;
mod fmt;
mod get;
mod job;
mod launchd;
mod logs;
mod preflight;
mod run_cmd;
mod status;

use std::ffi::OsString;
use std::process::ExitCode;

use agent_core::state::Store;
use anyhow::Result;
use clap::{CommandFactory, FromArgMatches};

use crate::cli::{Cli, Command, DaemonAction, GetResource};

/// One line for `beagle help`.
pub const ABOUT: &str =
    "Schedule and run coding agents: jobs, runs, logs, the daemon and a live TUI";

/// Run as `program` (`agentctl`, or `beagle agent` under the unified
/// command) with the arguments after the program name. `program` is what
/// help, usage and every hint name, and it decides what `daemon install`
/// puts in the launchd unit.
pub fn run_as(program: &'static str, args: Vec<OsString>) -> ExitCode {
    agent_core::ctl::set_name(program);
    let command_line = std::iter::once(OsString::from(program)).chain(args);
    let parsed = command(program)
        .try_get_matches_from(command_line)
        .and_then(|m| Cli::from_arg_matches(&m));
    let cli = match parsed {
        Ok(cli) => cli,
        Err(e) => {
            // --help and --version are "errors" that print to stdout and
            // exit 0; real usage errors exit 2.
            let _ = e.print();
            return ExitCode::from(u8::try_from(e.exit_code()).unwrap_or(2));
        }
    };
    match dispatch(cli, program) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("{program}: {e:#}");
            ExitCode::FAILURE
        }
    }
}

/// The clap command, renamed so help and usage say `program`.
fn command(program: &'static str) -> clap::Command {
    Cli::command().name(program).bin_name(program)
}

/// Whether this is the standalone `agentctl` rather than a host's name.
pub(crate) fn standalone() -> bool {
    agent_core::ctl::name() == agent_core::ctl::STANDALONE
}

fn dispatch(cli: Cli, program: &'static str) -> Result<ExitCode> {
    // `-j` is the job form and takes precedence; it needs no subcommand.
    if let Some(job) = &cli.job {
        job::run(
            &Store::open_default()?,
            job,
            &cli.param,
            cli.model.as_deref(),
        )?;
        return Ok(ExitCode::SUCCESS);
    }
    let Some(command) = cli.command else {
        // No subcommand and no -j: show help rather than silently succeeding.
        command(program).print_help()?;
        println!();
        return Ok(ExitCode::FAILURE);
    };
    match command {
        Command::Apply { manifests } => return apply::apply(manifests.as_deref()),
        Command::Get { resource } => {
            let store = Store::open_default()?;
            match resource {
                GetResource::Projects => get::projects(&store)?,
                GetResource::Jobs => get::jobs(&store)?,
                GetResource::Agents { project } => get::agents(&store, project.as_deref())?,
                GetResource::Runs {
                    project,
                    agent,
                    state,
                    limit,
                } => {
                    get::runs(&store, project, agent, state.as_deref(), limit)?;
                }
            }
        }
        Command::Run {
            target,
            parent,
            param,
            model,
        } => {
            run_cmd::run(
                &Store::open_default()?,
                &target,
                parent.as_deref(),
                &param,
                model.as_deref(),
            )?;
        }
        Command::Logs {
            target,
            follow,
            kind,
        } => {
            logs::logs(&Store::open_default()?, &target, follow, kind.as_deref())?;
        }
        Command::Resume { run_id } => run_cmd::resume(&Store::open_default()?, &run_id)?,
        Command::Kill { run_id } => run_cmd::kill(&Store::open_default()?, &run_id)?,
        Command::Attach { run_id } => run_cmd::attach(&Store::open_default()?, &run_id)?,
        Command::Status => status::status(&Store::open_default()?)?,
        Command::Top { args } => return Ok(agenttop::run_as(&format!("{program} top"), args)),
        Command::Errors {
            since,
            since_last,
            max_hours,
            json,
            write,
        } => errors::errors(
            &Store::open_default()?,
            since.as_deref(),
            since_last.as_deref(),
            max_hours,
            json,
            write.as_deref(),
        )?,
        Command::Daemon { action } => match action {
            DaemonAction::Install { print } => daemon::install(print)?,
            DaemonAction::Restart => daemon::restart()?,
            DaemonAction::Uninstall => daemon::uninstall()?,
            DaemonAction::Run { args } => {
                return Ok(agentd::run_as(&format!("{program} daemon run"), args));
            }
        },
    }
    Ok(ExitCode::SUCCESS)
}
