//! `agentctl` — the kubectl of the agentd stack.
//!
//! Reads and writes the shared `SQLite` store (and shells out to tmux for
//! kill/attach); never talks to the daemon directly.

mod apply;
mod cli;
mod fmt;
mod get;
mod job;
mod logs;
mod preflight;
mod run_cmd;
mod status;

use std::process::ExitCode;

use agent_core::state::Store;
use anyhow::Result;
use clap::Parser;

use crate::cli::{Cli, Command, GetResource};

fn main() -> ExitCode {
    match dispatch(Cli::parse()) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn dispatch(cli: Cli) -> Result<ExitCode> {
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
        <Cli as clap::CommandFactory>::command().print_help()?;
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
        Command::Kill { run_id } => run_cmd::kill(&Store::open_default()?, &run_id)?,
        Command::Attach { run_id } => run_cmd::attach(&Store::open_default()?, &run_id)?,
        Command::Status => status::status(&Store::open_default()?)?,
    }
    Ok(ExitCode::SUCCESS)
}
