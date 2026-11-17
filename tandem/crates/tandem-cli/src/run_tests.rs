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
        command,
    }
}

#[test]
fn hosted_update_points_at_the_host_updater() {
    let err = dispatch(&cli(Command::Update { check: true }), "beagle tandem").unwrap_err();
    assert_eq!(err, hosted_update_error("beagle tandem"));
    assert!(err.contains("run `beagle update`"), "{err}");
}

#[test]
fn hosted_names_still_run_the_store_commands() {
    // Not the update refusal: it gets as far as looking for the database.
    let err = dispatch(&cli(Command::Queue { all: false }), "beagle tandem").unwrap_err();
    assert!(err.contains("no database at"), "{err}");
}

#[test]
fn the_standalone_name_is_the_binary() {
    assert_eq!(STANDALONE, "tandem-cli");
}
