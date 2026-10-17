//! Bounded reads and atomic writes — the low-level filesystem discipline
//! every other `store` submodule goes through.

use std::fs;
use std::path::Path;

use crate::error::{Error, Result};

use super::MAX_FILE_BYTES;

/// Reads a file, returning `Ok(None)` if it does not exist and enforcing the
/// size cap before reading a byte of content.
pub(super) fn read_optional(path: &Path) -> Result<Option<String>> {
    match fs::metadata(path) {
        Ok(md) if md.len() > MAX_FILE_BYTES => {
            return Err(Error::FileTooLarge {
                path: path.to_owned(),
                size: md.len(),
                limit: MAX_FILE_BYTES,
            });
        }
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(Error::io(path, e)),
    }
    match fs::read_to_string(path) {
        Ok(content) => Ok(Some(content)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(Error::io(path, e)),
    }
}

/// As [`read_optional`], but an absent file is an error.
pub(super) fn read_bounded(path: &Path) -> Result<String> {
    read_optional(path)?.ok_or_else(|| {
        Error::io(
            path,
            std::io::Error::new(std::io::ErrorKind::NotFound, "file not found"),
        )
    })
}

/// Writes `content` to a sibling temp file (fsynced), then renames it into
/// place, so readers never observe a partial write.
pub(super) fn write_atomic(path: &Path, content: &str) -> Result<()> {
    beagle_utils::fs::write_atomic(path, content.as_bytes()).map_err(|e| Error::io(path, e))
}
