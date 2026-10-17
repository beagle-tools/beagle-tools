//! Unpacking an extension tarball and checking it is what it claims to be.

use std::path::Path;

use crate::error::{Error, Result};
use crate::manifest::ExtensionManifest;
use crate::tool;
use crate::version::Version;

/// The manifest file at the tarball root.
pub const MANIFEST: &str = "extension.toml";

/// Extracts `tarball` into `into` (which must exist).
pub fn extract(tarball: &Path, into: &Path) -> Result<()> {
    let args = [
        "-xzf".to_owned(),
        tarball.to_string_lossy().into_owned(),
        "-C".to_owned(),
        into.to_string_lossy().into_owned(),
    ];
    tool::run("tar", &args, tool::LOCAL_TIMEOUT).map(|_| ())
}

/// Validates an extracted tarball at `dir` for extension `name`, release
/// `version`, under base `base`: its manifest parses, names the right
/// extension and version, is compatible, and every declared binary is
/// present and executable.
pub fn validate(
    dir: &Path,
    name: &str,
    version: Version,
    base: Version,
) -> Result<ExtensionManifest> {
    let manifest_path = dir.join(MANIFEST);
    if !manifest_path.is_file() {
        return Err(Error::Manifest {
            path: manifest_path,
            message: "the tarball has no extension.toml at its root".to_owned(),
        });
    }
    let manifest = ExtensionManifest::load(&manifest_path)?;
    manifest.check_name(name)?;
    if manifest.version != version {
        return Err(Error::Manifest {
            path: manifest_path,
            message: format!(
                "declares version {} but was released as {version}",
                manifest.version
            ),
        });
    }
    manifest.check_compatible(base)?;
    for bin in manifest.all_bins() {
        let path = dir.join(bin);
        if !is_executable_file(&path) {
            return Err(Error::Manifest {
                path: manifest_path,
                message: format!("declares binary `{bin}`, which the tarball does not contain as an executable file"),
            });
        }
    }
    Ok(manifest)
}

/// A regular file (not a symlink) with an execute bit set.
pub fn is_executable_file(path: &Path) -> bool {
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return false;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        meta.is_file() && meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        meta.is_file()
    }
}
