//! Listing an app's published releases and choosing one.
//!
//! The monorepo shares one GitHub releases feed between apps, so every
//! listing is filtered by the extension's tag prefix. Releases come from the
//! GitHub API by default; `BEAGLE_RELEASES_URL` / `BEAGLE_DOWNLOAD_BASE`
//! point at a mirror instead (tests use `file://` fixtures this way).

use crate::error::{Error, Result};
use crate::registry::ExtensionSpec;
use crate::tool;
use crate::version::{self, Version};

const GITHUB_RELEASES: &str =
    "https://api.github.com/repos/beagle-tools/beagle-tools/releases?per_page=100";
const GITHUB_DOWNLOADS: &str = "https://github.com/beagle-tools/beagle-tools/releases/download";

/// One published release of one app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    /// The git tag, e.g. `beagle-rca-v0.29.0`.
    pub tag: String,
    /// The version parsed from the tag.
    pub version: Version,
    /// Marked as a pre-release on GitHub.
    pub prerelease: bool,
    /// Asset file names attached to the release.
    pub assets: Vec<String>,
}

impl Release {
    /// Whether the release carries `asset`.
    pub fn has_asset(&self, asset: &str) -> bool {
        self.assets.iter().any(|a| a == asset)
    }
}

/// Where release listings and downloads come from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseSource {
    /// URL returning the GitHub releases JSON array.
    pub releases_url: String,
    /// Base URL; an asset lives at `<base>/<tag>/<asset>`.
    pub download_base: String,
}

impl ReleaseSource {
    /// This repo's GitHub releases, unless overridden by
    /// `BEAGLE_RELEASES_URL` / `BEAGLE_DOWNLOAD_BASE`.
    pub fn from_env() -> Self {
        let var = |name: &str, default: &str| {
            std::env::var(name)
                .ok()
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| default.to_owned())
        };
        Self {
            releases_url: var("BEAGLE_RELEASES_URL", GITHUB_RELEASES),
            download_base: var("BEAGLE_DOWNLOAD_BASE", GITHUB_DOWNLOADS),
        }
    }

    /// The download URL of `asset` in release `tag`.
    pub fn asset_url(&self, tag: &str, asset: &str) -> String {
        format!("{}/{tag}/{asset}", self.download_base.trim_end_matches('/'))
    }

    /// Fetches `spec`'s releases, newest first (see [`parse_releases`]).
    pub fn fetch(&self, spec: &ExtensionSpec, include_pre: bool) -> Result<Vec<Release>> {
        let mut args = tool::curl_args();
        args.extend(
            [
                "-H",
                "Accept: application/vnd.github+json",
                "-H",
                "User-Agent: beagle",
                &self.releases_url,
            ]
            .map(str::to_owned),
        );
        let body = tool::run("curl", &args, tool::DOWNLOAD_TIMEOUT)?;
        parse_releases(
            &String::from_utf8_lossy(&body),
            spec.tag_prefix,
            include_pre,
        )
    }
}

/// Parses the GitHub releases JSON into `tag_prefix`'s releases, newest
/// first. Drafts are always skipped, pre-releases unless `include_pre`, and
/// tags that aren't `<prefix>X.Y.Z` are ignored.
pub fn parse_releases(json: &str, tag_prefix: &str, include_pre: bool) -> Result<Vec<Release>> {
    #[derive(serde::Deserialize)]
    struct RawAsset {
        name: String,
    }
    #[derive(serde::Deserialize)]
    struct Raw {
        tag_name: String,
        #[serde(default)]
        draft: bool,
        #[serde(default)]
        prerelease: bool,
        #[serde(default)]
        assets: Vec<RawAsset>,
    }
    let raw: Vec<Raw> = serde_json::from_str(json).map_err(|e| {
        let head: String = json.chars().take(120).collect();
        Error::ParseReleases(format!("{e} (body starts: {head})"))
    })?;
    let mut releases: Vec<Release> = raw
        .into_iter()
        .filter(|r| !r.draft && (include_pre || !r.prerelease))
        .filter_map(|r| {
            Some(Release {
                version: Version::from_tag(&r.tag_name, tag_prefix)?,
                tag: r.tag_name,
                prerelease: r.prerelease,
                assets: r.assets.into_iter().map(|a| a.name).collect(),
            })
        })
        .collect();
    releases.sort_by_key(|r| std::cmp::Reverse(r.version));
    Ok(releases)
}

/// The releases that carry a build of `spec` for `target`, newest first.
pub fn installable<'r>(
    releases: &'r [Release],
    spec: &ExtensionSpec,
    target: &str,
) -> Vec<&'r Release> {
    let asset = spec.asset_name(target);
    releases.iter().filter(|r| r.has_asset(&asset)).collect()
}

/// Picks the release to install: `requested` (any form
/// [`Version::parse_for`] accepts) or, when `None`, the newest installable
/// one. An unknown version lists the five nearest that exist.
pub fn choose<'r>(
    releases: &'r [Release],
    spec: &ExtensionSpec,
    target: &str,
    requested: Option<&str>,
) -> Result<&'r Release> {
    let candidates = installable(releases, spec, target);
    let Some(requested) = requested else {
        return candidates.first().copied().ok_or(Error::NotReleased {
            name: spec.name.to_owned(),
            target: target.to_owned(),
        });
    };
    let wanted = Version::parse_for(requested, spec.tag_prefix)?;
    if let Some(release) = candidates.iter().find(|r| r.version == wanted) {
        return Ok(release);
    }
    let known: Vec<Version> = candidates.iter().map(|r| r.version).collect();
    let near = version::nearest(wanted, &known, 5);
    let nearest = if near.is_empty() {
        String::new()
    } else {
        let list: Vec<String> = near.iter().map(ToString::to_string).collect();
        format!("; nearest: {}", list.join(", "))
    };
    Err(Error::NoSuchVersion {
        name: spec.name.to_owned(),
        version: wanted.to_string(),
        target: target.to_owned(),
        nearest,
    })
}

#[cfg(test)]
#[path = "tests/releases.rs"]
mod tests;
