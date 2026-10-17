//! Installing an extension version: staged download → sha256 → extract →
//! validate → rename into place → swap `current`.
//!
//! A failure at any step deletes the staging directory and leaves `current`
//! untouched, so a working install is never broken by a bad download. The
//! whole operation holds `locks/<ext>.lock`.
//!
//! Submodules: `download` (curl + sha256), `extract` (tar + manifest
//! checks), `swap` (current/prune/uninstall), `lock`.

pub(crate) mod download;
pub(crate) mod extract;
pub mod lock;
pub mod swap;

pub use download::{parse_sha, sha256_of};
pub use extract::{is_executable_file, MANIFEST};

use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::error::{Error, Result};
use crate::manifest::ExtensionManifest;
use crate::paths::Layout;
use crate::registry::ExtensionSpec;
use crate::releases::{Release, ReleaseSource};
use crate::version::Version;

/// The record the base writes beside each installed version.
pub const INSTALL_RECORD: &str = "install.toml";

/// Everything one install needs.
#[derive(Debug, Clone, Copy)]
pub struct Request<'a> {
    /// The extension.
    pub spec: &'a ExtensionSpec,
    /// The release to install.
    pub release: &'a Release,
    /// Where to download from.
    pub source: &'a ReleaseSource,
    /// This platform's target triple.
    pub target: &'a str,
    /// The on-disk layout.
    pub layout: &'a Layout,
    /// This base's version, for the manifest compatibility check.
    pub base_version: Version,
    /// How long to wait for another install to finish.
    pub lock_wait: Duration,
}

/// What an install did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installed {
    /// The version now active.
    pub version: Version,
    /// Its manifest.
    pub manifest: ExtensionManifest,
    /// True when that version was already on disk (only `current` moved).
    pub was_present: bool,
}

/// Installs `req.release` and makes it the active version.
pub fn install(req: &Request<'_>) -> Result<Installed> {
    let name = req.spec.name;
    let _lock = lock::acquire(&req.layout.lock_file(name), name, req.lock_wait)?;
    swap::remove_stale_staging(req.layout, name)?;

    let version = req.release.version;
    let version_dir = req.layout.version_dir(name, version);
    if let Ok(manifest) = ExtensionManifest::load(&version_dir.join(MANIFEST)) {
        swap::swap_current(req.layout, name, version)?;
        return Ok(Installed {
            version,
            manifest,
            was_present: true,
        });
    }

    let staging = req.layout.staging_dir(name, std::process::id());
    fs::create_dir_all(&staging).map_err(|e| Error::io(&staging, e))?;
    let result = stage(req, &staging).and_then(|manifest| {
        if version_dir.exists() {
            // A broken leftover (no readable manifest): replace it.
            fs::remove_dir_all(&version_dir).map_err(|e| Error::io(&version_dir, e))?;
        }
        let ready = staging.join("ready");
        fs::rename(&ready, &version_dir).map_err(|e| Error::io(&version_dir, e))?;
        swap::swap_current(req.layout, name, version)?;
        Ok(manifest)
    });
    let _ = fs::remove_dir_all(&staging); // best effort; the next run retries
    Ok(Installed {
        version,
        manifest: result?,
        was_present: false,
    })
}

/// Downloads, verifies and unpacks into `staging/ready/` laid out exactly
/// like a version directory.
fn stage(req: &Request<'_>, staging: &Path) -> Result<ExtensionManifest> {
    let asset = req.spec.asset_name(req.target);
    let url = req.source.asset_url(&req.release.tag, &asset);
    let tarball = staging.join(&asset);
    let sha_file = staging.join(format!("{asset}.sha256"));
    download::download(&url, &tarball)?;
    download::download(&format!("{url}.sha256"), &sha_file)?;
    let digest = download::verify(&tarball, &sha_file, &asset)?;

    let unpacked = staging.join("unpacked");
    fs::create_dir_all(&unpacked).map_err(|e| Error::io(&unpacked, e))?;
    extract::extract(&tarball, &unpacked)?;
    let manifest = extract::validate(
        &unpacked,
        req.spec.name,
        req.release.version,
        req.base_version,
    )?;

    let ready = staging.join("ready");
    let bin_dir = ready.join("bin");
    fs::create_dir_all(&bin_dir).map_err(|e| Error::io(&bin_dir, e))?;
    for bin in manifest.all_bins() {
        let dest = bin_dir.join(bin);
        fs::rename(unpacked.join(bin), &dest).map_err(|e| Error::io(&dest, e))?;
    }
    let dest = ready.join(MANIFEST);
    fs::rename(unpacked.join(MANIFEST), &dest).map_err(|e| Error::io(&dest, e))?;
    let record = install_record(&req.release.tag, &digest, &url, SystemTime::now());
    let dest = ready.join(INSTALL_RECORD);
    beagle_utils::fs::write_atomic(&dest, record.as_bytes()).map_err(|e| Error::io(&dest, e))?;
    Ok(manifest)
}

/// The text of `install.toml`.
pub fn install_record(tag: &str, sha256: &str, url: &str, at: SystemTime) -> String {
    let mut table = toml::Table::new();
    table.insert("tag".into(), tag.into());
    table.insert("sha256".into(), sha256.into());
    table.insert("source".into(), url.into());
    table.insert("installed_at".into(), rfc3339(at).into());
    table.to_string()
}

/// `at` as an RFC 3339 UTC timestamp (`2026-10-16T09:30:00Z`).
pub fn rfc3339(at: SystemTime) -> String {
    let secs = at.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let days = i64::try_from(secs / 86_400).unwrap_or(0);
    let rem = secs % 86_400;
    // Howard Hinnant's days-to-civil algorithm.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

#[cfg(test)]
#[path = "../tests/install.rs"]
mod tests;
