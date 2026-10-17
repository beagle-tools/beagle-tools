//! Release versions: plain `MAJOR.MINOR.PATCH`, tagged per app as
//! `<tag prefix>X.Y.Z` (e.g. `beagle-rca-v0.29.0`).

use std::fmt;
use std::str::FromStr;

use crate::error::{Error, Result};

/// A released version. Ordered so newer sorts greater.
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
    /// Builds a version from its three components.
    pub const fn new(major: u64, minor: u64, patch: u64) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// The version this binary was built as (release CI stamps it).
    pub fn current() -> Self {
        // CARGO_PKG_VERSION is always well-formed X.Y.Z for this crate.
        env!("CARGO_PKG_VERSION")
            .parse()
            .unwrap_or(Self::new(0, 0, 0))
    }

    /// Parses what a user types for an app tagged with `tag_prefix`:
    /// `0.29.0`, `v0.29.0` or the full tag `beagle-rca-v0.29.0`. Another
    /// app's tag (`beagle-agentd-v0.1.0` for rca) is rejected.
    pub fn parse_for(input: &str, tag_prefix: &str) -> Result<Self> {
        let trimmed = input.strip_prefix(tag_prefix).unwrap_or(input);
        let bare = trimmed.strip_prefix('v').unwrap_or(trimmed);
        bare.parse().map_err(|()| Error::InvalidVersion {
            input: input.to_owned(),
            prefix: tag_prefix.to_owned(),
        })
    }

    /// Parses a release tag for `tag_prefix`; `None` for any other app's tag
    /// or a non-`X.Y.Z` suffix (pre-release suffixes included).
    pub fn from_tag(tag: &str, tag_prefix: &str) -> Option<Self> {
        tag.strip_prefix(tag_prefix)?.parse().ok()
    }

    /// The release tag for this version under `tag_prefix`.
    pub fn tag(self, tag_prefix: &str) -> String {
        format!("{tag_prefix}{self}")
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl FromStr for Version {
    type Err = ();

    /// Strict `X.Y.Z` of decimal integers; nothing else.
    fn from_str(s: &str) -> std::result::Result<Self, ()> {
        let mut parts = s.split('.');
        let mut next = || {
            parts
                .next()
                .filter(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
                .and_then(|p| p.parse::<u64>().ok())
                .ok_or(())
        };
        let version = Self::new(next()?, next()?, next()?);
        if parts.next().is_some() {
            return Err(());
        }
        Ok(version)
    }
}

impl<'de> serde::Deserialize<'de> for Version {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        raw.parse()
            .map_err(|()| serde::de::Error::custom(format!("`{raw}` is not X.Y.Z")))
    }
}

impl serde::Serialize for Version {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

/// Up to `n` versions from `known` closest to `wanted`, in ascending order:
/// what an "unknown version" error suggests.
pub fn nearest(wanted: Version, known: &[Version], n: usize) -> Vec<Version> {
    let mut sorted = known.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    let at = sorted.partition_point(|v| *v < wanted);
    let lo = at.saturating_sub(n);
    let hi = (at + n).min(sorted.len());
    let mut window: Vec<(usize, Version)> = sorted[lo..hi]
        .iter()
        .enumerate()
        .map(|(i, v)| ((lo + i).abs_diff(at), *v))
        .collect();
    // Distance in release order; ties go to the newer version.
    window.sort_by_key(|(dist, v)| (*dist, std::cmp::Reverse(*v)));
    let mut picked: Vec<Version> = window.into_iter().take(n).map(|(_, v)| v).collect();
    picked.sort_unstable();
    picked
}

#[cfg(test)]
#[path = "tests/version.rs"]
mod tests;
