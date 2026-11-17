//! Size-capped, degrade-don't-die file access for the store.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use super::{Diagram, MAX_FILE_BYTES};
use crate::error::{Error, Result};

/// Reads a size-capped file. `Ok(None)` when it doesn't exist.
pub(super) fn read_capped(path: &Path, limit: u64) -> Result<Option<String>> {
    let meta = match fs::metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(Error::io(path, e)),
    };
    if meta.len() > limit {
        return Err(Error::FileTooLarge {
            path: path.to_owned(),
            size: meta.len(),
            limit,
        });
    }
    let bytes = fs::read(path).map_err(|e| Error::io(path, e))?;
    // Invalid UTF-8 is replaced, never fatal.
    Ok(Some(String::from_utf8_lossy(&bytes).into_owned()))
}

pub(super) fn read_text(path: &Path, limit: u64, warnings: &mut Vec<String>) -> String {
    match read_capped(path, limit) {
        Ok(text) => text.unwrap_or_default(),
        Err(e) => {
            warnings.push(e.to_string());
            String::new()
        }
    }
}

pub(super) fn read_toml<T: serde::de::DeserializeOwned>(path: &Path) -> Result<Option<T>> {
    let Some(text) = read_capped(path, MAX_FILE_BYTES)? else {
        return Ok(None);
    };
    toml::from_str(&text)
        .map(Some)
        .map_err(|e| Error::ParseToml {
            path: path.to_owned(),
            source: Box::new(e),
        })
}

pub(super) fn read_diagrams(dir: &Path, warnings: &mut Vec<String>) -> Vec<Diagram> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(std::result::Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "txt"))
        .collect();
    paths.sort();
    paths
        .into_iter()
        .map(|p| Diagram {
            name: p
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default(),
            body: read_text(&p, MAX_FILE_BYTES, warnings),
        })
        .collect()
}

pub(super) fn create_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path).map_err(|e| Error::io(path, e))
}

/// Writes via a temp file + rename so a half-written file is never seen.
pub(super) fn write_atomic(path: &Path, contents: &str) -> Result<()> {
    beagle_utils::fs::write_atomic(path, contents.as_bytes()).map_err(|e| Error::io(path, e))
}
