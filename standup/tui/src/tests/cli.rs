//! Tests for `cli`.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::*;

fn p(args: &[&str]) -> Result<Args, String> {
    parse(args.iter().map(|s| (*s).to_owned()))
}

#[test]
fn bare_invocation_opens_the_tui() {
    let a = p(&[]).unwrap();
    assert_eq!(a.command, Command::Tui { date: None });
    let a = p(&["--root", "/r", "--date", "2026-10-27"]).unwrap();
    assert_eq!(a.root.as_deref(), Some("/r"));
    assert_eq!(
        a.command,
        Command::Tui {
            date: Some("2026-10-27".to_owned())
        }
    );
}

#[test]
fn ticket_collects_repeated_flags() {
    let a = p(&[
        "ticket", "ABC-123", "--title", "Gateway", "--system", "gateway", "--system", "sched",
        "--pr", "u1", "--update", "--root", "/r",
    ])
    .unwrap();
    let Command::Ticket {
        id,
        title,
        systems,
        prs,
        update,
        ..
    } = a.command
    else {
        panic!("not a ticket command");
    };
    assert_eq!(id, "ABC-123");
    assert_eq!(title.as_deref(), Some("Gateway"));
    assert_eq!(systems, ["gateway", "sched"]);
    assert_eq!(prs, ["u1"]);
    assert!(update);
}

#[test]
fn errors_are_reported_not_ignored() {
    assert!(p(&["nope"]).is_err());
    assert!(p(&["list", "--bogus", "x"]).is_err());
    assert!(p(&["new", "--since"]).is_err());
    assert!(p(&["ticket"]).is_err());
    assert!(p(&["show", "a", "b"]).is_err());
    assert!(p(&["list", "--update"]).is_err());
    assert_eq!(
        p(&["list", "--json"]).unwrap().command,
        Command::List { json: true }
    );
    assert_eq!(p(&["--help"]).unwrap().command, Command::Help);
    assert_eq!(p(&["guide"]).unwrap().command, Command::Guide);
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

#[test]
fn root_help_version_and_unknown_words() {
    for argv in [
        &["--root", "/x", "list"][..],
        &["list", "--root", "/x"],
        &["--root=/x", "list"],
    ] {
        assert_eq!(p(argv).unwrap().root.as_deref(), Some("/x"), "{argv:?}");
    }
    for word in ["help", "-h", "--help"] {
        assert_eq!(p(&[word]).unwrap().command, Command::Help, "{word}");
    }
    for word in ["version", "-V", "--version"] {
        assert_eq!(p(&[word]).unwrap().command, Command::Version, "{word}");
    }
    assert_eq!(p(&["lsit"]).unwrap_err(), "unknown command `lsit`");
}
