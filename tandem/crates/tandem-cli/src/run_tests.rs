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
fn hosted_update_is_a_usage_error() {
    let code = crate::run_as("beagle tandem", vec!["update".into(), "--check".into()]);
    assert_eq!(code, std::process::ExitCode::from(2));
    assert!(hosted_update_error("beagle tandem").starts_with("unknown command `update`"));
}

#[test]
fn hosted_names_still_run_the_store_commands() {
    // Not the update refusal: it gets as far as looking for the database.
    let err = dispatch_cli(&cli(Command::AppQueue { all: false }), "beagle tandem").unwrap_err();
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

#[test]
fn the_desktop_queue_commands_are_app_prefixed_and_keep_old_aliases() {
    for word in ["app-queue", "queue"] {
        assert!(
            matches!(
                parse(&[word]).command,
                Some(Command::AppQueue { all: false })
            ),
            "{word}"
        );
    }
    for word in ["app-review", "review"] {
        assert!(
            matches!(
                parse(&[word, "--repo", "acme/widgets", "--pr", "7"]).command,
                Some(Command::AppReview { pr: 7, .. })
            ),
            "{word}"
        );
    }
}

#[test]
fn the_help_names_both_queues() {
    use clap::CommandFactory as _;
    let help = Cli::command().render_long_help().to_string();
    assert!(help.contains("TWO review queues"), "{help}");
    assert!(
        help.contains("app-review") && help.contains("app-queue"),
        "{help}"
    );
    // The old names stay as hidden aliases only.
    assert!(!help.contains("\n  queue "), "{help}");
    assert!(!help.contains("\n  review "), "{help}");
}
