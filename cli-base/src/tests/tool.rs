//! Tests for the timeout-bounded tool runner, using `sh` fixtures.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::*;

#[test]
fn returns_stdout_on_success() {
    let out = run("sh", &["-c", "printf hello"], LOCAL_TIMEOUT).unwrap();
    assert_eq!(out, b"hello");
}

#[test]
fn carries_stderr_on_failure() {
    let err = run("sh", &["-c", "echo nope >&2; exit 3"], LOCAL_TIMEOUT).unwrap_err();
    assert_eq!(err.to_string(), "sh failed: nope");
}

#[test]
fn reports_the_status_when_stderr_is_empty() {
    let err = run("sh", &["-c", "exit 4"], LOCAL_TIMEOUT).unwrap_err();
    assert!(err.to_string().contains("exited with"), "{err}");
}

#[test]
fn kills_a_tool_that_outlives_its_timeout() {
    // `exec` so the kill hits the sleeping process itself, not a wrapper.
    let err = run("sh", &["-c", "exec sleep 30"], Duration::ZERO).unwrap_err();
    assert!(err.to_string().contains("timed out"), "{err}");
}

#[test]
fn missing_tool_is_a_tool_error() {
    let err = run::<&str>("definitely-not-a-real-tool", &[], LOCAL_TIMEOUT).unwrap_err();
    assert!(err.to_string().contains("not found on PATH"), "{err}");
}
