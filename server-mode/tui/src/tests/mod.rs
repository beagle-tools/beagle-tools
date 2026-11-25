//! Parsing, state files and formatting. `session.rs` runs the commands
//! against fake programs; nothing here touches the real system.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

mod session;

use std::path::PathBuf;

use crate::cli::{hours_to_secs, parse, Cmd, OnOpts};
use crate::state::{fmt_dur, legacy_dir, tail, StateDir, SESSION_FILES};

fn words(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_owned()).collect()
}

fn on(list: &[&str]) -> OnOpts {
    match parse(&words(list)) {
        Ok(Cmd::On(opts)) => opts,
        other => panic!("{list:?} → {other:?}"),
    }
}

#[test]
fn no_arguments_and_bare_flags_mean_on() {
    assert_eq!(parse(&[]), Ok(Cmd::On(OnOpts::default())));
    assert_eq!(on(&["on"]), OnOpts::default());
    let opts = on(&["--no-lock", "-t", "2"]);
    assert!(!opts.lock);
    assert_eq!(opts.secs, Some(7200));
}

#[test]
fn on_takes_every_flag_of_the_script() {
    let opts = on(&[
        "on",
        "-t",
        "0.01",
        "--start-up",
        "-f",
        "run.sh",
        "--no-lock",
    ]);
    assert_eq!(opts.hours.as_deref(), Some("0.01"));
    assert_eq!(opts.secs, Some(36));
    assert_eq!(opts.file, Some(PathBuf::from("run.sh")));
    assert!(!opts.lock);
    let long = on(&["on", "--hours", "8", "--startup", "--file", "a.sh"]);
    assert_eq!(long.secs, Some(8 * 3600));
    assert_eq!(long.file, Some(PathBuf::from("a.sh")));
    let inline = on(&["on", "--hours=1.5", "--file=b.sh"]);
    assert_eq!(inline.secs, Some(5400));
    assert_eq!(inline.file, Some(PathBuf::from("b.sh")));
}

#[test]
fn the_other_commands_parse() {
    for (word, cmd) in [
        ("off", Cmd::Off),
        ("status", Cmd::Status),
        ("log", Cmd::Log),
        ("lock", Cmd::Lock),
        ("help", Cmd::Help),
    ] {
        assert_eq!(parse(&words(&[word])), Ok(cmd), "{word}");
    }
    assert_eq!(parse(&words(&["on", "--help"])), Ok(Cmd::Help));
    assert_eq!(parse(&words(&["-h"])), Ok(Cmd::Help));
}

#[test]
fn usage_errors_name_the_problem() {
    let err = |list: &[&str]| parse(&words(list)).err().unwrap_or_default();
    assert_eq!(
        err(&["nap"]),
        "unknown command: nap (see beagle server-mode --help)"
    );
    assert_eq!(
        err(&["on", "--lid"]),
        "unknown option: --lid (see beagle server-mode --help)"
    );
    assert_eq!(err(&["on", "-t"]), "-t needs a number of hours");
    assert_eq!(err(&["on", "--file"]), "--file needs a file");
    assert_eq!(
        err(&["off", "now"]),
        "unexpected argument: now (see beagle server-mode --help)"
    );
    assert_eq!(
        err(&["on", "-t", "soon"]),
        "--hours must be a positive number, got 'soon'"
    );
}

#[test]
fn hours_must_be_positive_and_round_down_to_seconds() {
    assert_eq!(hours_to_secs("1"), Ok(3600));
    assert_eq!(hours_to_secs("0.5"), Ok(1800));
    assert_eq!(hours_to_secs("0.0001"), Ok(1), "never below one second");
    for bad in ["0", "-1", "abc", "", "inf", "NaN"] {
        assert!(hours_to_secs(bad).is_err(), "{bad}");
    }
}

#[test]
fn durations_format_like_the_script() {
    assert_eq!(fmt_dur(0), "0h00m");
    assert_eq!(fmt_dur(59), "0h00m");
    assert_eq!(fmt_dur(3900), "1h05m");
    assert_eq!(fmt_dur(30 * 3600 + 60), "30h01m");
    assert_eq!(fmt_dur(-5), "0h00m");
}

#[test]
fn state_files_hold_one_trimmed_value_each() {
    let tmp = tempfile::tempdir().unwrap();
    let state = StateDir::new(tmp.path().join("nested/server-mode"));
    assert_eq!(state.read("since"), None);
    state.write("since", 1_700_000_000_i64).unwrap();
    state.write("caffeinate.pid", 4242).unwrap();
    state.write("file", "/tmp/run.sh").unwrap();
    std::fs::write(state.path("timer.pid"), "\n").unwrap();
    std::fs::write(state.path("startup.pid"), "0\n").unwrap();
    assert_eq!(state.read_i64("since"), Some(1_700_000_000));
    assert_eq!(state.read_pid("caffeinate.pid"), Some(4242));
    assert_eq!(state.read("file").as_deref(), Some("/tmp/run.sh"));
    assert_eq!(state.read_pid("timer.pid"), None, "empty is unset");
    assert_eq!(state.read_pid("startup.pid"), None, "pid 0 is never real");

    std::fs::write(state.path("startup.log"), "kept\n").unwrap();
    state.clear();
    for name in SESSION_FILES {
        assert!(!state.path(name).exists(), "{name}");
    }
    assert!(state.has_log(), "off keeps the log for `log`");
}

#[test]
fn the_legacy_dir_is_the_scripts_path() {
    assert_eq!(
        legacy_dir(std::path::Path::new("/home/me")),
        PathBuf::from("/home/me/.local/state/server-mode")
    );
}

#[test]
fn tail_keeps_the_last_lines() {
    let text = (1..=60).fold(String::new(), |acc, i| acc + &format!("line {i}\n"));
    let out = tail(&text, 50);
    assert_eq!(out.lines().count(), 50);
    assert!(out.starts_with("line 11\n"));
    assert!(out.ends_with("line 60\n"));
    assert_eq!(tail("", 50), "");
    assert_eq!(tail("one", 50), "one\n");
}

#[test]
fn lock_delay_reads_sysadminctl_output() {
    use crate::session::lock_delay;
    assert_eq!(
        lock_delay("screenLock delay is 5 seconds").as_deref(),
        Some("5 seconds")
    );
    assert_eq!(
        lock_delay("screenLock delay is immediate").as_deref(),
        Some("immediate")
    );
    assert_eq!(lock_delay("screenLock is off").as_deref(), Some("off"));
    assert_eq!(lock_delay("something else"), None);
}

#[test]
fn help_and_usage_errors_exit_like_every_beagle_tool() {
    use std::ffi::OsString;
    use std::process::ExitCode;
    let run = |list: &[&str]| crate::run(list.iter().map(OsString::from).collect());
    assert_eq!(run(&["--help"]), ExitCode::SUCCESS);
    assert_eq!(run(&["on", "-h"]), ExitCode::SUCCESS);
    assert_eq!(run(&["nap"]), ExitCode::from(2));
    assert_eq!(run(&["on", "-t", "0"]), ExitCode::from(2));
}
