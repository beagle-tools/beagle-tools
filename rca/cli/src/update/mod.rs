//! Self-update of the standalone rca `beagle` binary: rca's release
//! channel over the shared updater (`beagle_utils::update`: curl, sha256
//! verify, tar, atomic same-directory rename) and picker
//! (`beagle_tui::version_picker`).
//!
//! Only the standalone binary updates itself. Linked into the unified
//! `beagle` command (`beagle rca …`), an rca-only download would replace
//! every other tool, so there `beagle update` (the unified updater) owns
//! updating — see `crate::run`.
//!
//! Versions are plain `MAJOR.MINOR.PATCH`; picking what to install is the
//! user's call (`--version`, or the interactive `version list`), and moving
//! to an older version is as supported as moving to a newer one.

use std::path::Path;

pub use beagle_utils::update::{release_target, Version};

use beagle_utils::update::Channel;

use crate::error::{Error, Result};

/// The repository URL from `Cargo.toml`.
pub const REPO_URL: &str = env!("CARGO_PKG_REPOSITORY");

/// Every rca release is tagged `beagle-rca-vX.Y.Z`; other apps in the
/// monorepo (`beagle-tandem-v…`, `beagle-cli-v…`) are ignored.
pub const TAG_PREFIX: &str = "beagle-rca-v";

/// The GitHub repository releases come from, as `owner/name`. Derived from
/// the crate metadata so a fork updates itself from the fork.
#[must_use]
pub fn repo_slug() -> &'static str {
    REPO_URL
        .strip_prefix("https://github.com/")
        .unwrap_or(REPO_URL)
}

/// rca's release channel: `beagle-rca-<target>.tar.gz` holding `beagle`
/// and `beagle-agentd` at its root (the agentd is swapped too when it is
/// installed beside the CLI, so the two never drift apart).
#[must_use]
pub fn channel() -> Channel {
    Channel {
        repo: repo_slug(),
        tag_prefix: TAG_PREFIX,
        asset_prefix: "beagle-rca",
        binary: "beagle",
        companions: &["beagle-agentd"],
    }
}

/// The version this binary was built as.
#[must_use]
pub fn current() -> Version {
    Version::from_pkg(env!("CARGO_PKG_VERSION"))
}

/// Parses a `--version` value: `0.2.0`, `v0.2.0`, or `beagle-rca-v0.2.0`.
///
/// # Errors
/// A message naming the bad input when it is not `X.Y.Z`.
pub fn parse_version(input: &str) -> std::result::Result<Version, String> {
    channel().parse_version(input)
}

/// Every published rca release, newest first.
///
/// # Errors
/// [`Error::Tool`] if `curl` is missing or fails, [`Error::ParseReleases`]
/// if the feed is not the expected JSON.
pub fn fetch_releases() -> Result<Vec<Version>> {
    Ok(channel().fetch_releases()?)
}

/// Downloads `version` for this platform, verifies its sha256 and
/// atomically replaces the binary at `exe` — upgrade or downgrade alike.
///
/// # Errors
/// [`Error::UnsupportedTarget`] on platforms without prebuilt binaries, and
/// [`Error::Tool`] / [`Error::ChecksumMismatch`] / [`Error::Io`] for
/// download, verification and install failures.
pub fn update_to(version: Version, exe: &Path) -> Result<()> {
    Ok(channel().update_to(version, exe)?)
}

/// The interactive `beagle version list` picker; `None` when cancelled.
///
/// # Errors
/// [`Error::Terminal`] on draw or input failures.
pub fn pick_version(releases: &[Version], current: Version) -> Result<Option<Version>> {
    Ok(beagle_tui::version_picker::pick_version(
        "beagle",
        &channel(),
        releases,
        current,
    )?)
}

impl From<beagle_utils::update::Error> for Error {
    fn from(err: beagle_utils::update::Error) -> Self {
        use beagle_utils::update::Error as U;
        match err {
            U::Tool { tool, message } => Self::Tool { tool, message },
            U::ParseReleases(message) => Self::ParseReleases(message),
            U::ChecksumMismatch {
                path,
                expected,
                actual,
            } => Self::ChecksumMismatch {
                path,
                expected,
                actual,
            },
            U::UnsupportedTarget => Self::UnsupportedTarget {
                target: beagle_utils::update::TARGET_DESCRIPTION,
                repo: REPO_URL,
            },
            U::Io { path, source } => Self::Io { path, source },
        }
    }
}

#[cfg(test)]
#[path = "tests/version.rs"]
mod tests;
