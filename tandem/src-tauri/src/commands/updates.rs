//! App version management: list Tandem's own published releases and
//! install a chosen one (upgrade or rollback). Installs go through the
//! Tauri updater, so every bundle is signature-checked against the
//! updater public key regardless of which direction the version moves.

use serde::{Deserialize, Serialize};
use tandem_core::TandemError;
use tauri_plugin_updater::UpdaterExt;

/// Where Tandem's release feed lives (the app's own repo — unrelated to
/// whichever repos the user reviews).
const RELEASES_REPO: &str = "beagle-tools/beagle-tools";

/// Tandem's tags in that monorepo (`tandem-vX.Y.Z`); other components'
/// releases share the feed and are filtered out by this prefix.
const TAG_PREFIX: &str = "tandem-v";

/// One published app version, newest first.
#[derive(Debug, Clone, Serialize)]
pub struct AppRelease {
    pub version: String,
    pub tag: String,
    pub published_at: Option<String>,
    /// Whether this release carries an updater manifest — releases
    /// without one (e.g. manually uploaded) can't be installed in-app.
    pub installable: bool,
}

#[derive(Debug, Deserialize)]
struct WireRelease {
    tag_name: String,
    published_at: Option<String>,
    draft: bool,
    prerelease: bool,
    assets: Vec<WireAsset>,
}

#[derive(Debug, Deserialize)]
struct WireAsset {
    name: String,
}

fn feed_err(context: &str, e: impl std::fmt::Display) -> TandemError {
    TandemError::Config(format!(
        "{context}: {e} — check your network connection and try again"
    ))
}

#[tauri::command]
pub async fn list_app_releases() -> Result<Vec<AppRelease>, TandemError> {
    let url = format!("https://api.github.com/repos/{RELEASES_REPO}/releases?per_page=50");
    let response = reqwest::Client::new()
        .get(&url)
        .header("User-Agent", "tandem-app")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| feed_err("couldn't reach the release feed", e))?;
    let status = response.status();
    if !status.is_success() {
        return Err(TandemError::GithubApi {
            status: status.as_u16(),
            message: format!("release feed request to {url} failed"),
        });
    }
    let releases: Vec<WireRelease> = response
        .json()
        .await
        .map_err(|e| feed_err("couldn't parse the release feed", e))?;

    Ok(releases
        .into_iter()
        .filter(|r| !r.draft && !r.prerelease)
        .filter_map(|r| {
            let version = r.tag_name.strip_prefix(TAG_PREFIX)?.to_owned();
            Some(AppRelease {
                version,
                installable: r.assets.iter().any(|a| a.name == "latest.json"),
                tag: r.tag_name,
                published_at: r.published_at,
            })
        })
        .collect())
}

/// Install a specific released version — newer or older than the one
/// running. The frontend relaunches the app after this returns.
#[tauri::command]
pub async fn install_app_version(app: tauri::AppHandle, tag: String) -> Result<(), TandemError> {
    let valid = tag
        .strip_prefix(TAG_PREFIX)
        .is_some_and(|rest| rest.split('.').all(|p| p.parse::<u64>().is_ok()));
    if !valid {
        return Err(TandemError::Config(format!(
            "invalid release tag: {tag} — expected a {TAG_PREFIX}X.Y.Z tag from the releases list"
        )));
    }
    let manifest =
        format!("https://github.com/{RELEASES_REPO}/releases/download/{tag}/latest.json");
    let endpoint = tauri::Url::parse(&manifest)
        .map_err(|e| TandemError::Config(format!("bad update manifest url {manifest}: {e}")))?;

    let updater = app
        .updater_builder()
        .endpoints(vec![endpoint])
        .map_err(|e| TandemError::Config(format!("update endpoint rejected: {e}")))?
        // Allow rollbacks: any version different from the running one.
        .version_comparator(|current, update| update.version != current)
        .build()
        .map_err(|e| TandemError::Config(format!("updater unavailable: {e}")))?;

    let update = updater
        .check()
        .await
        .map_err(|e| {
            TandemError::Config(format!(
                "couldn't get {tag}: {e} — if this release was published in the \
                 last few minutes it may still be building; try again shortly"
            ))
        })?
        .ok_or_else(|| TandemError::Config(format!("{tag} is already the installed version")))?;

    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|e| feed_err(&format!("install of {tag} failed"), e))?;
    Ok(())
}
