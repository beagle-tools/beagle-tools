//! Switching, pruning and removing installed versions.
//!
//! `current` is only ever replaced by renaming a fresh symlink over it, so a
//! reader always sees either the old or the new target — never no link.

use std::fs;
use std::path::Path;

use crate::error::{Error, Result};
use crate::paths::{Layout, STAGING_PREFIX};
use crate::version::Version;

/// Points `ext`'s `current` at `version` atomically.
pub fn swap_current(layout: &Layout, ext: &str, version: Version) -> Result<()> {
    let link = layout.current_link(ext);
    let tmp = layout
        .ext_dir(ext)
        .join(format!("current.tmp-{}", std::process::id()));
    let _ = fs::remove_file(&tmp); // leftover from a crashed swap
    symlink(Path::new(&version.to_string()), &tmp)?;
    fs::rename(&tmp, &link).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        Error::io(&link, e)
    })
}

/// `extensions use`: switches to an installed version, without network.
pub fn use_version(layout: &Layout, ext: &str, version: Version) -> Result<()> {
    if !layout.version_dir(ext, version).is_dir() {
        return Err(Error::VersionNotInstalled {
            name: ext.to_owned(),
            version: version.to_string(),
        });
    }
    swap_current(layout, ext, version)
}

/// Deletes all but `keep` non-current versions of `ext` (newest kept).
/// The current version is never deleted. Returns what was removed.
pub fn prune(layout: &Layout, ext: &str, keep: usize) -> Result<Vec<Version>> {
    let current = layout.current_version(ext);
    let mut removed = Vec::new();
    let others = layout
        .installed_versions(ext)
        .into_iter()
        .filter(|v| Some(*v) != current);
    for version in others.skip(keep) {
        let dir = layout.version_dir(ext, version);
        fs::remove_dir_all(&dir).map_err(|e| Error::io(&dir, e))?;
        removed.push(version);
    }
    Ok(removed)
}

/// Removes `ext`'s active version and its `current` link, or the whole
/// extension directory with `all_versions`. Returns the versions left.
pub fn uninstall(layout: &Layout, ext: &str, all_versions: bool) -> Result<Vec<Version>> {
    let dir = layout.ext_dir(ext);
    if !dir.exists() {
        return Err(Error::NotInstalled {
            name: ext.to_owned(),
        });
    }
    if all_versions {
        fs::remove_dir_all(&dir).map_err(|e| Error::io(&dir, e))?;
        return Ok(Vec::new());
    }
    let current = layout
        .current_version(ext)
        .ok_or_else(|| Error::NotInstalled {
            name: ext.to_owned(),
        })?;
    // Link first: an interrupted uninstall leaves "not installed", never a
    // `current` pointing at a half-deleted directory.
    let link = layout.current_link(ext);
    fs::remove_file(&link).map_err(|e| Error::io(&link, e))?;
    let version_dir = layout.version_dir(ext, current);
    fs::remove_dir_all(&version_dir).map_err(|e| Error::io(&version_dir, e))?;
    Ok(layout.installed_versions(ext))
}

/// Deletes `.staging-*` directories left by interrupted installs. Only
/// called while holding the install lock, so none of them is live.
pub fn remove_stale_staging(layout: &Layout, ext: &str) -> Result<()> {
    let dir = layout.ext_dir(ext);
    let Ok(entries) = fs::read_dir(&dir) else {
        return Ok(()); // nothing installed yet
    };
    for entry in entries.filter_map(std::result::Result::ok) {
        let name = entry.file_name();
        let is_staging = name.to_str().is_some_and(|n| n.starts_with(STAGING_PREFIX));
        if is_staging {
            let path = entry.path();
            fs::remove_dir_all(&path).map_err(|e| Error::io(&path, e))?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn symlink(target: &Path, link: &Path) -> Result<()> {
    std::os::unix::fs::symlink(target, link).map_err(|e| Error::io(link, e))
}

#[cfg(not(unix))]
fn symlink(_target: &Path, link: &Path) -> Result<()> {
    Err(Error::io(
        link,
        std::io::Error::new(std::io::ErrorKind::Unsupported, "symlinks need unix"),
    ))
}
