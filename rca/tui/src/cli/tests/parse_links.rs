//! Tests for the `pr` and `ticket` argument parsers (`cli::links`).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use std::path::PathBuf;

use crate::cli::{parse_args, Command};

fn parse(argv: &[&str]) -> Result<Command, String> {
    parse_args(argv.iter().map(ToString::to_string))
}

#[test]
fn pr_parses_add_and_list() {
    match parse(&[
        "pr",
        "add",
        "my-rca",
        "https://github.com/o/r/pull/1",
        "--root",
        "/x",
    ]) {
        Ok(Command::PrAdd { root, id, url }) => {
            assert_eq!(root, Some(PathBuf::from("/x")));
            assert_eq!(id.as_str(), "my-rca");
            assert_eq!(url, "https://github.com/o/r/pull/1");
        }
        other => panic!("unexpected parse: {other:?}"),
    }
    assert!(matches!(
        parse(&["pr", "list", "my-rca"]),
        Ok(Command::PrList { .. })
    ));
    assert!(parse(&["pr"]).is_err(), "missing subcommand");
    assert!(parse(&["pr", "close", "my-rca"]).is_err(), "bad subcommand");
    assert!(parse(&["pr", "add", "my-rca"]).is_err(), "missing url");
    assert!(parse(&["pr", "list", "my-rca", "--frob"]).is_err());
}

#[test]
fn ticket_parses_add_list_and_sync() {
    match parse(&["ticket", "add", "my-rca", "ENG-42", "--root", "/x"]) {
        Ok(Command::TicketAdd {
            root,
            id,
            reference,
        }) => {
            assert_eq!(root, Some(PathBuf::from("/x")));
            assert_eq!(id.as_str(), "my-rca");
            assert_eq!(reference, "ENG-42");
        }
        other => panic!("unexpected parse: {other:?}"),
    }
    assert!(matches!(
        parse(&["ticket", "list", "my-rca"]),
        Ok(Command::TicketList { .. })
    ));
    assert!(matches!(
        parse(&["ticket", "sync"]),
        Ok(Command::TicketSync { id: None, .. })
    ));
    assert!(parse(&["ticket"]).is_err(), "missing subcommand");
    assert!(
        parse(&["ticket", "close", "my-rca"]).is_err(),
        "bad subcommand"
    );
    assert!(parse(&["ticket", "add", "my-rca"]).is_err(), "missing ref");
    assert!(parse(&["ticket", "list"]).is_err(), "list needs an id");
}
