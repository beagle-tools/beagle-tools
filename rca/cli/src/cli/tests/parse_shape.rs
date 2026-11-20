//! The command-line shape every beagle tool shares (task: one CLI shape):
//! `--root` anywhere, `help`, `version`, and unknown words.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::*;

fn parse(argv: &[&str]) -> Result<Command, String> {
    parse_args(argv.iter().map(ToString::to_string))
}

fn root_of(argv: &[&str]) -> Option<PathBuf> {
    match parse(argv).unwrap() {
        Command::List { root, .. } | Command::Tui { root } => root,
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn root_works_before_or_after_the_command_and_with_equals() {
    let want = Some(PathBuf::from("/x"));
    assert_eq!(root_of(&["--root", "/x", "list"]), want);
    assert_eq!(root_of(&["list", "--root", "/x"]), want);
    assert_eq!(root_of(&["--root=/x", "list"]), want);
    assert_eq!(root_of(&["list", "--root=/x", "--json"]), want);
    assert_eq!(root_of(&["--root=/x"]), want);
    assert!(parse(&["--root"]).is_err(), "--root needs a value");
    assert!(parse(&["list", "--root"]).is_err());
}

#[test]
fn help_and_version_parse() {
    for word in ["--help", "-h"] {
        assert!(matches!(parse(&[word]), Ok(Command::Help)), "{word}");
    }
    for word in ["--version", "-V", "version"] {
        assert!(matches!(parse(&[word]), Ok(Command::Version)), "{word}");
    }
}

#[test]
fn unknown_words_are_errors() {
    assert_eq!(
        parse(&["lsit"]).unwrap_err(),
        "unknown command `lsit`".to_owned()
    );
    assert!(parse(&["list", "--bogus"]).is_err());
}
