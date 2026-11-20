//! The GitHub CLI (`gh`): one bounded way to run it, and a PR's state.
//!
//! Every tool that asks GitHub something through `gh` goes through here, so
//! `gh` is always found on the augmented `PATH` (a GUI- or launchd-started
//! process inherits only the minimal system `PATH`), always runs under a
//! timeout, and fails with a message a user can act on. Callers keep their
//! own poll intervals and their own parsing of anything beyond a PR's state.
//!
//! Needs the `gh` feature (it parses `gh`'s JSON with `serde_json`).

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Duration;

use crate::process;

mod pr;

pub use pr::{parse_pr_state, PrState};

/// A sensible bound on one `gh` API call (`pr view`, `api`, `auth token`).
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Why a `gh` call failed. [`fmt::Display`] gives the actionable message.
#[derive(Debug)]
pub enum Error {
    /// `gh` could not be started: not installed ([`io::ErrorKind::NotFound`])
    /// or another launch failure.
    Launch(io::Error),
    /// `gh` did not finish in time; it was killed. The error names the
    /// command and the bound.
    TimedOut(io::Error),
    /// `gh` ran and exited non-zero; this is its trimmed stderr.
    Failed(String),
    /// `gh` succeeded but its output was not what was asked for.
    Unexpected(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Launch(e) if e.kind() == io::ErrorKind::NotFound => write!(
                f,
                "can't run `gh` ({e}) — is the GitHub CLI installed? (brew install gh)"
            ),
            Self::Launch(e) => write!(f, "can't run `gh`: {e}"),
            Self::TimedOut(e) => write!(f, "{e} — is GitHub reachable?"),
            Self::Failed(stderr) => f.write_str(stderr),
            Self::Unexpected(what) => write!(f, "unexpected output from gh: {what}"),
        }
    }
}

impl std::error::Error for Error {}

/// `gh` as found on the augmented `PATH`, or plain `gh` (left to the OS)
/// when it isn't there.
#[must_use]
pub fn program() -> PathBuf {
    process::which("gh").unwrap_or_else(|| "gh".into())
}

/// A [`Command`] for [`program`] whose child sees the augmented `PATH`.
#[must_use]
pub fn command() -> Command {
    process::command(program())
}

/// Whether `gh` is installed and `gh --version` succeeds within `timeout`.
#[must_use]
pub fn available(timeout: Duration) -> bool {
    process::which("gh").is_some_and(|gh| {
        output_from(&gh, &["--version"], timeout).is_ok_and(|out| out.status.success())
    })
}

/// Runs `gh args` bounded by `timeout` and returns its raw output, whatever
/// the exit status.
///
/// # Errors
/// [`Error::Launch`] when `gh` can't start, [`Error::TimedOut`] on expiry.
pub fn output(args: &[&str], timeout: Duration) -> Result<Output, Error> {
    output_from(&program(), args, timeout)
}

/// Runs `gh args` bounded by `timeout` and returns its stdout (untrimmed).
///
/// # Errors
/// As [`output`], plus [`Error::Failed`] (with `gh`'s stderr) on a non-zero
/// exit.
pub fn run(args: &[&str], timeout: Duration) -> Result<String, Error> {
    run_from(&program(), args, timeout)
}

/// Asks `gh pr view` where a PR stands. `pr` is anything `gh pr view`
/// takes: a URL, or a number with `-R` given through `repo`.
///
/// # Errors
/// As [`run`], plus [`Error::Unexpected`] when the JSON has no known state.
pub fn pr_state(pr: &str, repo: Option<&str>, timeout: Duration) -> Result<PrState, Error> {
    pr_state_from(&program(), pr, repo, timeout)
}

fn pr_state_from(
    gh: &Path,
    pr: &str,
    repo: Option<&str>,
    timeout: Duration,
) -> Result<PrState, Error> {
    let mut args = vec!["pr", "view", pr, "--json", "state,isDraft"];
    if let Some(repo) = repo {
        args.extend(["-R", repo]);
    }
    let json = run_from(gh, &args, timeout)?;
    parse_pr_state(&json).ok_or_else(|| Error::Unexpected(json.trim().to_owned()))
}

fn output_from(gh: &Path, args: &[&str], timeout: Duration) -> Result<Output, Error> {
    let mut cmd = process::command(gh);
    cmd.args(args);
    process::output_with_timeout(&mut cmd, timeout).map_err(|e| {
        if e.kind() == io::ErrorKind::TimedOut {
            Error::TimedOut(e)
        } else {
            Error::Launch(e)
        }
    })
}

fn run_from(gh: &Path, args: &[&str], timeout: Duration) -> Result<String, Error> {
    let out = output_from(gh, args, timeout)?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(Error::Failed(
            String::from_utf8_lossy(&out.stderr).trim().to_owned(),
        ))
    }
}

#[cfg(test)]
mod tests;
