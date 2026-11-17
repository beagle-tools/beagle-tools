//! Tests for `cli`.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::*;

fn p(args: &[&str]) -> Result<Args, String> {
    parse(args.iter().map(|s| (*s).to_owned()))
}

#[test]
fn tui_with_and_without_a_target() {
    assert_eq!(p(&[]).unwrap().command, Command::Tui { target: None });
    assert_eq!(
        p(&["widgets-42"]).unwrap().command,
        Command::Tui {
            target: Some("widgets-42".to_owned())
        }
    );
    let a = p(&["--root", "/r", "o/r#1"]).unwrap();
    assert_eq!(a.root.as_deref(), Some("/r"));
}

#[test]
fn new_refresh_show_parse() {
    assert_eq!(
        p(&["new", "o/r#1", "--ticket-url", "https://l/ABC-1"])
            .unwrap()
            .command,
        Command::New {
            pr: "o/r#1".to_owned(),
            ticket: None,
            ticket_url: Some("https://l/ABC-1".to_owned())
        }
    );
    assert_eq!(
        p(&["refresh", "r-1"]).unwrap().command,
        Command::Refresh {
            target: "r-1".to_owned(),
            ticket_url: None
        }
    );
    assert_eq!(
        p(&["show", "r-1", "--diff"]).unwrap().command,
        Command::Show {
            target: "r-1".to_owned(),
            diff: true
        }
    );
    assert_eq!(
        p(&["list", "--json"]).unwrap().command,
        Command::List {
            json: true,
            archived: false
        }
    );
    assert_eq!(p(&["guide"]).unwrap().command, Command::Guide);
    assert_eq!(p(&["sync"]).unwrap().command, Command::Sync);
    assert_eq!(
        p(&["comments", "r-1"]).unwrap().command,
        Command::Comments {
            target: "r-1".to_owned()
        }
    );
}

#[test]
fn errors_are_reported_not_ignored() {
    assert!(p(&["new"]).is_err());
    assert!(p(&["show"]).is_err());
    assert!(p(&["bogus"]).is_err());
    assert!(p(&["list", "--diff"]).is_err());
    assert!(p(&["archive"]).is_err());
    assert!(p(&["list", "--nope", "x"]).is_err());
    assert!(p(&["path", "a-1", "b-2"]).is_err());
}

#[test]
fn os_args_parse_and_help_runs() {
    let args = vec![std::ffi::OsString::from("--help")];
    assert_eq!(parse_os(args.clone()).unwrap().command, Command::Help);
    assert_eq!(crate::run(args), std::process::ExitCode::SUCCESS);
    assert_eq!(
        crate::run(vec![std::ffi::OsString::from("nope")]),
        std::process::ExitCode::from(2)
    );
}
