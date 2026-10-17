//! App version management: list rca's own published releases and install a
//! chosen one (upgrade or rollback). Installs go through the Tauri updater,
//! so every bundle is signature-checked against the updater public key in
//! `tauri.conf.json` whichever direction the version moves. The routine
//! "is there something newer" check runs in the frontend against the
//! `beagle-rca-latest` mirror (`plugins.updater.endpoints`).

use serde::Deserialize;
use tauri_plugin_updater::UpdaterExt;

use crate::dto::AppRelease;
use crate::error::{Error, Result};

/// The monorepo every app's releases are published from.
const RELEASES_REPO: &str = "beagle-tools/beagle-tools";

/// rca's versioned tags in that monorepo (`beagle-rca-vX.Y.Z`); other apps'
/// releases and the rolling `beagle-rca-latest` mirror share the feed and
/// are filtered out by this prefix.
const TAG_PREFIX: &str = "beagle-rca-v";

/// The `X.Y.Z` an rca release tag carries, or `None` when the tag isn't one
/// of rca's versioned releases.
pub(crate) fn tag_version(tag: &str) -> Option<&str> {
    let version = tag.strip_prefix(TAG_PREFIX)?;
    let parts: Vec<&str> = version.split('.').collect();
    let numeric = |p: &&str| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit());
    (parts.len() == 3 && parts.iter().all(numeric)).then_some(version)
}

#[derive(Debug, Deserialize)]
struct WireRelease {
    tag_name: String,
    published_at: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<WireAsset>,
}

#[derive(Debug, Deserialize)]
struct WireAsset {
    name: String,
}

/// Turns GitHub's releases JSON into rca's installable versions: published
/// (no drafts or prereleases), `beagle-rca-v*` only, in feed order (GitHub
/// lists newest first). Pure so it is testable without a network.
pub(crate) fn parse_releases(json: &str) -> Result<Vec<AppRelease>> {
    let releases: Vec<WireRelease> = serde_json::from_str(json)
        .map_err(|e| Error::Update(format!("couldn't parse the release feed: {e}")))?;
    Ok(releases
        .into_iter()
        .filter(|r| !r.draft && !r.prerelease)
        .filter_map(|r| {
            let version = tag_version(&r.tag_name)?.to_owned();
            Some(AppRelease {
                version,
                installable: r.assets.iter().any(|a| a.name == "latest.json"),
                tag: r.tag_name,
                published_at: r.published_at,
            })
        })
        .collect())
}

/// The updater manifest inside one versioned release.
pub(crate) fn manifest_url(tag: &str) -> String {
    format!("https://github.com/{RELEASES_REPO}/releases/download/{tag}/latest.json")
}

fn network(context: &str, e: impl std::fmt::Display) -> Error {
    Error::Update(format!(
        "{context}: {e} — check your network connection and try again"
    ))
}

/// Every published rca desktop release, newest first.
#[tauri::command]
pub async fn list_app_releases() -> Result<Vec<AppRelease>> {
    let url = format!("https://api.github.com/repos/{RELEASES_REPO}/releases?per_page=100");
    let response = reqwest::Client::new()
        .get(&url)
        .header("User-Agent", "beagle-desktop")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| network("couldn't reach the release feed", e))?;
    let status = response.status();
    if !status.is_success() {
        return Err(Error::Update(format!(
            "release feed request to {url} failed with HTTP {status}"
        )));
    }
    let body = response
        .text()
        .await
        .map_err(|e| network("couldn't read the release feed", e))?;
    parse_releases(&body)
}

/// Installs a specific released version — newer or older than the one
/// running. The frontend relaunches the app after this returns.
#[tauri::command]
pub async fn install_app_version(app: tauri::AppHandle, tag: String) -> Result<()> {
    if tag_version(&tag).is_none() {
        return Err(Error::Update(format!(
            "invalid release tag: {tag} — expected a {TAG_PREFIX}X.Y.Z tag from the releases list"
        )));
    }
    let manifest = manifest_url(&tag);
    let endpoint = tauri::Url::parse(&manifest)
        .map_err(|e| Error::Update(format!("bad update manifest url {manifest}: {e}")))?;

    let updater = app
        .updater_builder()
        .endpoints(vec![endpoint])
        .map_err(|e| Error::Update(format!("update endpoint rejected: {e}")))?
        // Allow rollbacks: any version different from the running one.
        .version_comparator(|current, update| update.version != current)
        .build()
        .map_err(|e| Error::Update(format!("updater unavailable: {e}")))?;

    let update = updater
        .check()
        .await
        .map_err(|e| {
            Error::Update(format!(
                "couldn't get {tag}: {e} — if this release was published in the \
                 last few minutes it may still be building; try again shortly"
            ))
        })?
        .ok_or_else(|| Error::Update(format!("{tag} is already the installed version")))?;

    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|e| network(&format!("install of {tag} failed"), e))?;
    Ok(())
}

#[cfg(test)]
#[path = "tests/updates.rs"]
mod tests;
