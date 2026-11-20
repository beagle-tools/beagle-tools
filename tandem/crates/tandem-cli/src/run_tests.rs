//! Tests for the entry points: the program name a host passes to
//! [`run_as`](crate::run_as), and who owns `update`.

#![allow(clippy::unwrap_used)] // panicking is the correct failure mode in tests

use std::path::PathBuf;

use crate::{dispatch, hosted_update_error, Cli, Command, STANDALONE};

fn cli(command: Command) -> Cli {
    Cli {
        db: Some(PathBuf::from("/nonexistent/tandem.sqlite3")),
        dev: false,
        pretty: false,
        command: Some(command),
    }
}

/// Runs `cli` through `dispatch` the way `run_as` does for a store command.
fn dispatch_cli(cli: &Cli, program: &str) -> Result<(), String> {
    let Some(command) = &cli.command else {
        return Err("no command".to_owned());
    };
    dispatch(cli, command, program)
}

#[test]
fn hosted_update_points_at_the_host_updater() {
    let err = dispatch_cli(&cli(Command::Update { check: true }), "beagle tandem").unwrap_err();
    assert_eq!(err, hosted_update_error("beagle tandem"));
    assert!(err.contains("run `beagle update`"), "{err}");
}

#[test]
fn hosted_names_still_run_the_store_commands() {
    // Not the update refusal: it gets as far as looking for the database.
    let err = dispatch_cli(&cli(Command::Queue { all: false }), "beagle tandem").unwrap_err();
    assert!(err.contains("no database at"), "{err}");
}

#[test]
fn the_standalone_name_is_the_binary() {
    assert_eq!(STANDALONE, "tandem-cli");
}

fn parse(args: &[&str]) -> Cli {
    use clap::Parser as _;
    Cli::try_parse_from(std::iter::once("tandem-cli").chain(args.iter().copied())).unwrap()
}

#[test]
fn no_command_means_the_review_tui() {
    assert!(parse(&[]).command.is_none());
}

#[test]
fn the_review_queue_commands_join_the_cli() {
    let cli = parse(&["add", "acme/widgets#7", "--no-kick"]);
    assert!(matches!(
        cli.command,
        Some(Command::ReviewQueue(tandem_tui::Command::Add { ref prs, no_kick: true, .. }))
            if prs == &["acme/widgets#7"]
    ));
    for args in [
        &["claim", "--out", "/tmp/x"][..],
        &["ingest", "--run", "r", "--file", "f"],
        &["fail", "--run", "r", "--error", "e"],
        &["list"],
    ] {
        assert!(
            matches!(parse(args).command, Some(Command::ReviewQueue(_))),
            "{args:?}"
        );
    }
    // The existing commands are unchanged.
    assert!(matches!(parse(&["paths"]).command, Some(Command::Paths)));
}
