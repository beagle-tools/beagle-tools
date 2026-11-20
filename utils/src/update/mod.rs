//! Self-update against GitHub releases: list a channel's versions, then
//! download, verify and atomically swap in a release binary.
//!
//! Only with the `update` feature (it parses the releases feed with
//! `serde_json`). Networking shells out to `curl`, checksums to
//! `shasum`/`sha256sum` and extraction to `tar`, so no HTTP or TLS stack is
//! linked. Every downloaded tarball is verified against the sha256 the
//! release workflow publishes before a single byte replaces the installed
//! binary, and the final swap is an atomic same-directory rename: an
//! interrupted update never leaves a half-written executable.
//!
//! A [`Channel`] names one app's releases in the monorepo: its tag prefix
//! (`beagle-rca-v`, `beagle-cli-v`), its tarball prefix and the binaries in
//! the tarball. Versions are plain `MAJOR.MINOR.PATCH`; moving to an older
//! version is as supported as moving to a newer one.
//!
//! Submodules: `error` ([`Error`]) and `install` (download, verify, swap).

mod error;
mod install;

pub use error::Error;
pub use install::install;

use std::fmt;
use std::path::Path;
use std::str::FromStr;

use install::curl_stdout;

/// Result alias for this module.
pub type Result<T> = std::result::Result<T, Error>;

/// One app's release line in a (possibly shared) GitHub repository.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Channel {
    /// The repository releases come from, as `owner/name`.
    pub repo: &'static str,
    /// Every release of this channel is tagged `<tag_prefix>X.Y.Z`, e.g.
    /// `beagle-rca-v`; other apps' tags in the same repo are ignored.
    pub tag_prefix: &'static str,
    /// Tarballs are `<asset_prefix>-<target>.tar.gz` (+ `.sha256`).
    pub asset_prefix: &'static str,
    /// The binary at the tarball root that replaces the running one.
    pub binary: &'static str,
    /// Other binaries in the tarball, swapped too when one already sits
    /// next to the running binary (so a CLI and its daemon never drift).
    pub companions: &'static [&'static str],
}

impl Channel {
    /// `https://github.com/<repo>`.
    #[must_use]
    pub fn repo_url(&self) -> String {
        format!("https://github.com/{}", self.repo)
    }

    /// The human releases page, for "nothing published yet" messages.
    #[must_use]
    pub fn releases_page(&self) -> String {
        format!("{}/releases", self.repo_url())
    }

    /// The git tag for `version`, e.g. `beagle-rca-v0.2.0`.
    #[must_use]
    pub fn tag(&self, version: Version) -> String {
        format!("{}{version}", self.tag_prefix)
    }

    /// Parses one of this channel's tags; `None` for any other app's tag
    /// or a non-`X.Y.Z` suffix (rolling `-latest` mirrors included).
    #[must_use]
    pub fn from_tag(&self, tag: &str) -> Option<Version> {
        tag.strip_prefix(self.tag_prefix)?.parse().ok()
    }

    /// Parses what a user types for `--version`: `0.2.0`, `v0.2.0`, or
    /// this channel's full tag.
    ///
    /// # Errors
    /// A message naming the bad input when it is not `X.Y.Z`.
    pub fn parse_version(&self, input: &str) -> std::result::Result<Version, String> {
        input.strip_prefix(self.tag_prefix).unwrap_or(input).parse()
    }

    /// The tarball for `target`, e.g. `beagle-rca-aarch64-apple-darwin.tar.gz`.
    #[must_use]
    pub fn asset_name(&self, target: &str) -> String {
        format!("{}-{target}.tar.gz", self.asset_prefix)
    }

    /// The download URL of `asset` in the release for `version`.
    #[must_use]
    pub fn asset_url(&self, version: Version, asset: &str) -> String {
        format!(
            "{}/releases/download/{}/{asset}",
            self.repo_url(),
            self.tag(version)
        )
    }

    /// Every published (non-draft, non-prerelease) release of this channel,
    /// newest first. Reads the newest 100 releases of the repository.
    ///
    /// # Errors
    /// [`Error::Tool`] if `curl` is missing or the request fails,
    /// [`Error::ParseReleases`] if the response is not the expected JSON.
    pub fn fetch_releases(&self) -> Result<Vec<Version>> {
        let url = format!(
            "https://api.github.com/repos/{}/releases?per_page=100",
            self.repo
        );
        let json = curl_stdout(
            &[
                "-fsSL",
                "-H",
                "Accept: application/vnd.github+json",
                "-H",
                "User-Agent: beagle",
                &url,
            ],
            install::LISTING_MAX_SECS,
        )?;
        self.parse_releases(&json)
    }

    /// Parses the GitHub releases JSON, keeping this channel's published
    /// versions, newest first. Split from [`Self::fetch_releases`] so it is
    /// testable without a network.
    ///
    /// # Errors
    /// [`Error::ParseReleases`] when the body is not a JSON array of releases.
    pub fn parse_releases(&self, json: &str) -> Result<Vec<Version>> {
        #[derive(serde::Deserialize)]
        struct Raw {
            tag_name: String,
            #[serde(default)]
            draft: bool,
            #[serde(default)]
            prerelease: bool,
        }
        let raw: Vec<Raw> = serde_json::from_str(json).map_err(|e| {
            let head: String = json.chars().take(120).collect();
            Error::ParseReleases(format!("{e} (body starts: {head})"))
        })?;
        let mut versions: Vec<Version> = raw
            .iter()
            .filter(|r| !r.draft && !r.prerelease)
            .filter_map(|r| self.from_tag(&r.tag_name))
            .collect();
        versions.sort_by_key(|v| std::cmp::Reverse(*v));
        Ok(versions)
    }

    /// Downloads `version` for this platform, verifies it and atomically
    /// replaces the binary at `exe` (and any companions beside it).
    ///
    /// # Errors
    /// [`Error::UnsupportedTarget`] on platforms without prebuilt binaries,
    /// else as for [`install`].
    pub fn update_to(&self, version: Version, exe: &Path) -> Result<()> {
        let target = release_target().ok_or(Error::UnsupportedTarget)?;
        let asset = self.asset_name(target);
        install(
            &self.asset_url(version, &asset),
            &self.asset_url(version, &format!("{asset}.sha256")),
            exe,
            self.binary,
            self.companions,
        )
    }
}

/// A released version: `MAJOR.MINOR.PATCH`. Ordered so newer sorts greater.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version {
    /// Incompatible-change counter.
    pub major: u64,
    /// Feature counter.
    pub minor: u64,
    /// Fix counter.
    pub patch: u64,
}

impl Version {
    /// Parses a crate's `CARGO_PKG_VERSION`; `0.0.0` if it is not `X.Y.Z`
    /// (a pre-release suffix, say), which sorts below every release.
    #[must_use]
    pub fn from_pkg(pkg_version: &str) -> Self {
        pkg_version.parse().unwrap_or(Self {
            major: 0,
            minor: 0,
            patch: 0,
        })
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl FromStr for Version {
    type Err = String;

    /// Accepts `X.Y.Z` with an optional leading `v`.
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let bare = s.strip_prefix('v').unwrap_or(s);
        let mut parts = bare.split('.');
        let mut next = |name: &str| {
            parts
                .next()
                .and_then(|p| p.parse::<u64>().ok())
                .ok_or_else(|| format!("invalid version `{s}`: bad {name} (expected X.Y.Z)"))
        };
        let version = Self {
            major: next("major")?,
            minor: next("minor")?,
            patch: next("patch")?,
        };
        if parts.next().is_some() {
            return Err(format!("invalid version `{s}`: too many components"));
        }
        Ok(version)
    }
}

/// The compile-time target triple, exactly as the release workflow names
/// its assets. `None` on platforms the release workflow does not build for.
#[must_use]
pub fn release_target() -> Option<&'static str> {
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        Some("aarch64-apple-darwin")
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        Some("x86_64-apple-darwin")
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Some("x86_64-unknown-linux-musl")
    } else {
        None
    }
}

/// A human description of the build platform, for "no prebuilt binary"
/// messages.
pub const TARGET_DESCRIPTION: &str = if cfg!(target_os = "macos") {
    "this macOS architecture"
} else if cfg!(target_os = "linux") {
    "this Linux architecture"
} else {
    "this platform"
};

#[cfg(test)]
#[path = "tests/version.rs"]
mod tests;
