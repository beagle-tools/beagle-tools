//! App version management: list Agentdesk's own published releases and
//! install a chosen one (upgrade or rollback). Installs go through the
//! Tauri updater, so every bundle is signature-checked against the updater
//! public key whichever direction the version moves.
//!
//! The "is there a newer version" pill does not come through here: the
//! frontend asks the updater plugin directly, against the rolling
//! `beagle-workflows-latest` endpoint in `tauri.conf.json`.

use serde::{Deserialize, Serialize};
use tauri_plugin_updater::UpdaterExt;

use crate::error::AppError;

/// The monorepo whose release feed carries Agentdesk's builds.
const RELEASES_REPO: &str = "beagle-tools/beagle-tools";

/// Agentdesk ships as part of workflows, tagged `beagle-workflows-vX.Y.Z`.
/// The other apps' releases and the rolling `beagle-workflows-latest`
/// mirror share the feed and are filtered out by this prefix.
const TAG_PREFIX: &str = "beagle-workflows-v";

/// One published app version, newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AppRelease {
    pub version: String,
    pub tag: String,
    pub published_at: Option<String>,
    /// Whether the release carries an updater manifest. One without (a
    /// CLI-only or hand-uploaded release) cannot be installed in-app.
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

/// The `X.Y.Z` a workflows release tag carries, or `None` for any tag that
/// is not one of its versioned releases.
fn tag_version(tag: &str) -> Option<&str> {
    let version = tag.strip_prefix(TAG_PREFIX)?;
    let parts: Vec<&str> = version.split('.').collect();
    let numeric = |p: &&str| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit());
    (parts.len() == 3 && parts.iter().all(numeric)).then_some(version)
}

/// Keep only published, versioned workflows releases, in feed order
/// (GitHub returns newest first).
fn app_releases(feed: Vec<WireRelease>) -> Vec<AppRelease> {
    feed.into_iter()
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
        .collect()
}

fn feed_err(context: &str, e: impl std::fmt::Display) -> AppError {
    AppError::new(format!(
        "{context}: {e} — check your network connection and try again"
    ))
}

/// Agentdesk's published versions, newest first.
#[tauri::command]
pub async fn list_app_releases() -> Result<Vec<AppRelease>, AppError> {
    let url = format!("https://api.github.com/repos/{RELEASES_REPO}/releases?per_page=100");
    let response = reqwest::Client::new()
        .get(&url)
        .header("User-Agent", "agentdesk")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| feed_err("couldn't reach the release feed", e))?;
    let status = response.status();
    if !status.is_success() {
        return Err(AppError::new(format!(
            "the release feed answered {status} — GitHub may be rate-limiting; try again in a few minutes"
        )));
    }
    let feed: Vec<WireRelease> = response
        .json()
        .await
        .map_err(|e| feed_err("couldn't parse the release feed", e))?;
    Ok(app_releases(feed))
}

/// Install a specific released version, newer or older than the running
/// one. The frontend relaunches the app after this returns.
#[tauri::command]
pub async fn install_app_version(app: tauri::AppHandle, tag: String) -> Result<(), AppError> {
    if tag_version(&tag).is_none() {
        return Err(AppError::new(format!(
            "invalid release tag: {tag} — expected a {TAG_PREFIX}X.Y.Z tag from the releases list"
        )));
    }
    let manifest =
        format!("https://github.com/{RELEASES_REPO}/releases/download/{tag}/latest.json");
    let endpoint = tauri::Url::parse(&manifest)
        .map_err(|e| AppError::new(format!("bad update manifest url {manifest}: {e}")))?;

    let updater = app
        .updater_builder()
        .endpoints(vec![endpoint])
        .map_err(|e| AppError::new(format!("update endpoint rejected: {e}")))?
        // Allow rollbacks: any version different from the running one.
        .version_comparator(|current, update| update.version != current)
        .build()
        .map_err(|e| AppError::new(format!("updater unavailable: {e}")))?;

    let update = updater
        .check()
        .await
        .map_err(|e| {
            AppError::new(format!(
                "couldn't get {tag}: {e} — if it was published in the last few minutes \
                 it may still be building; try again shortly"
            ))
        })?
        .ok_or_else(|| AppError::new(format!("{tag} is already the installed version")))?;

    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|e| feed_err(&format!("install of {tag} failed"), e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{app_releases, tag_version, AppRelease, WireRelease};

    #[test]
    fn accepts_workflows_release_tags() {
        assert_eq!(tag_version("beagle-workflows-v0.1.2"), Some("0.1.2"));
        assert_eq!(tag_version("beagle-workflows-v10.20.30"), Some("10.20.30"));
    }

    #[test]
    fn rejects_other_apps_mirrors_and_malformed_tags() {
        for tag in [
            "beagle-workflows-latest",
            "beagle-tandem-v1.2.3",
            "beagle-rca-v1.2.3",
            "workflows-v1.2.3",
            "beagle-workflows-v1.2",
            "beagle-workflows-v1.2.3.4",
            "beagle-workflows-v1..3",
            "beagle-workflows-v1.2.3-rc1",
            "beagle-workflows-v+1.2.3",
        ] {
            assert_eq!(tag_version(tag), None, "{tag}");
        }
    }

    #[test]
    fn feed_keeps_published_workflows_releases_in_order() {
        let feed: Vec<WireRelease> = serde_json::from_str(
            r#"[
              {"tag_name":"beagle-workflows-v0.3.0","published_at":"2026-10-02T00:00:00Z",
               "draft":false,"prerelease":false,"assets":[{"name":"latest.json"}]},
              {"tag_name":"beagle-workflows-latest","published_at":null,
               "draft":false,"prerelease":false,"assets":[{"name":"latest.json"}]},
              {"tag_name":"beagle-tandem-v9.0.0","published_at":null,
               "draft":false,"prerelease":false,"assets":[]},
              {"tag_name":"beagle-workflows-v0.4.0","published_at":null,
               "draft":true,"prerelease":false,"assets":[]},
              {"tag_name":"beagle-workflows-v0.2.0","published_at":null,
               "draft":false,"prerelease":false,"assets":[{"name":"beagle-workflows-aarch64-apple-darwin.tar.gz"}]}
            ]"#,
        )
        .unwrap_or_default();
        assert_eq!(
            app_releases(feed),
            vec![
                AppRelease {
                    version: "0.3.0".into(),
                    tag: "beagle-workflows-v0.3.0".into(),
                    published_at: Some("2026-10-02T00:00:00Z".into()),
                    installable: true,
                },
                AppRelease {
                    version: "0.2.0".into(),
                    tag: "beagle-workflows-v0.2.0".into(),
                    published_at: None,
                    installable: false,
                },
            ]
        );
    }
}
