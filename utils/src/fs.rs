//! Crash-safe file writes and optional reads.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

/// Writes `contents` to `path` atomically: readers see either the old file
/// or the complete new one, never a partial write.
///
/// The data goes to an exclusively-created hidden sibling
/// (`.<name>.tmp<N>`, same directory so the rename stays on one
/// filesystem), is `fsync`ed, then renamed over `path`. The temp file is
/// removed if any step fails. New files get the platform default mode.
pub fn write_atomic(path: impl AsRef<Path>, contents: &[u8]) -> io::Result<()> {
    write_atomic_inner(path.as_ref(), contents, None)
}

/// [`write_atomic`], with the file's permission bits set to `mode` (e.g.
/// `0o600`) *before* the rename, so the target never appears with looser
/// permissions. `mode` is not subject to the umask. Ignored on non-unix.
pub fn write_atomic_with_mode(
    path: impl AsRef<Path>,
    contents: &[u8],
    mode: u32,
) -> io::Result<()> {
    write_atomic_inner(path.as_ref(), contents, Some(mode))
}

/// The file's contents, or `None` if it does not exist. Every other error
/// (permissions, invalid UTF-8, `path` is a directory) is returned.
pub fn read_optional(path: impl AsRef<Path>) -> io::Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(s) => Ok(Some(s)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

fn write_atomic_inner(path: &Path, contents: &[u8], mode: Option<u32>) -> io::Result<()> {
    let parent = match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    let (file, tmp) = create_temp_sibling(parent, path, mode)?;
    let result = fill_and_rename(file, &tmp, path, contents, mode);
    if result.is_err() {
        // Best effort: the original error is more useful than a cleanup one.
        let _ = fs::remove_file(&tmp);
    }
    result
}

fn fill_and_rename(
    mut file: File,
    tmp: &Path,
    path: &Path,
    contents: &[u8],
    mode: Option<u32>,
) -> io::Result<()> {
    file.write_all(contents)?;
    set_mode(&file, mode)?;
    file.sync_all()?;
    drop(file);
    fs::rename(tmp, path)
}

#[cfg(unix)]
fn set_mode(file: &File, mode: Option<u32>) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    match mode {
        Some(mode) => file.set_permissions(fs::Permissions::from_mode(mode)),
        None => Ok(()),
    }
}

#[cfg(not(unix))]
fn set_mode(_file: &File, _mode: Option<u32>) -> io::Result<()> {
    Ok(())
}

/// Creates an exclusive temp file next to `target`, named after it so a
/// leftover from a crash is easy to trace back.
fn create_temp_sibling(
    parent: &Path,
    target: &Path,
    mode: Option<u32>,
) -> io::Result<(File, PathBuf)> {
    let base = target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    for attempt in 0u32..1024 {
        let candidate = parent.join(format!(".{base}.tmp{attempt}"));
        let mut opts = OpenOptions::new();
        opts.write(true).create_new(true);
        restrict_create_mode(&mut opts, mode);
        match opts.open(&candidate) {
            Ok(file) => return Ok((file, candidate)),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "no free temp file name",
    ))
}

/// With an explicit mode, create the temp file owner-only so it is never
/// briefly readable by others; [`set_mode`] applies the exact mode later.
#[cfg(unix)]
fn restrict_create_mode(opts: &mut OpenOptions, mode: Option<u32>) {
    use std::os::unix::fs::OpenOptionsExt as _;
    if mode.is_some() {
        opts.mode(0o600);
    }
}

#[cfg(not(unix))]
fn restrict_create_mode(_opts: &mut OpenOptions, _mode: Option<u32>) {}

#[cfg(test)]
// Tests fail loudly on setup errors; unwrap is the clearest way to do that.
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn entries(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn creates_and_replaces_without_leftovers() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        write_atomic(&path, b"one").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "one");
        write_atomic(&path, b"two").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "two");
        assert_eq!(entries(dir.path()), vec!["state.json"]);
    }

    #[test]
    fn skips_existing_temp_names() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("f");
        fs::write(dir.path().join(".f.tmp0"), b"stale").unwrap();
        write_atomic(&path, b"new").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "new");
        assert_eq!(entries(dir.path()), vec![".f.tmp0", "f"]);
    }

    #[test]
    fn missing_parent_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nope").join("f");
        let err = write_atomic(&path, b"x").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn failed_rename_cleans_up_temp() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("d");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("child"), b"x").unwrap();
        // Renaming a file over a non-empty directory fails on every platform.
        assert!(write_atomic(&target, b"x").is_err());
        assert_eq!(entries(dir.path()), vec!["d"]);
    }

    #[cfg(unix)]
    #[test]
    fn with_mode_sets_permissions() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("secret");
        write_atomic_with_mode(&path, b"k", 0o600).unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        write_atomic_with_mode(&path, b"k2", 0o640).unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o640);
        assert_eq!(fs::read_to_string(&path).unwrap(), "k2");
    }

    #[test]
    fn read_optional_maps_not_found_to_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("f");
        assert_eq!(read_optional(&path).unwrap(), None);
        fs::write(&path, "hi").unwrap();
        assert_eq!(read_optional(&path).unwrap().as_deref(), Some("hi"));
    }

    #[test]
    fn read_optional_propagates_other_errors() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read_optional(dir.path()).is_err());
    }
}
