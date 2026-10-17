//! Small pure helpers behind manifest loading — split from `manifest.rs`
//! for file size. `parse_schedule` is re-exported there.

use std::path::{Path, PathBuf};
use std::str::FromStr;

use cron::Schedule;

use crate::paths::expand_tilde;
use crate::CoreError;

/// Parse a cron expression, accepting the standard 5-field form (the `cron`
/// crate wants 6/7 fields with seconds, so `0` seconds is prepended).
pub fn parse_schedule(expr: &str) -> Result<Schedule, CoreError> {
    let fields = expr.split_whitespace().count();
    let normalized = if fields == 5 {
        format!("0 {expr}")
    } else {
        expr.to_owned()
    };
    Schedule::from_str(&normalized).map_err(|e| CoreError::Schedule {
        expr: expr.to_owned(),
        message: e.to_string(),
    })
}

/// Tilde-expand and manifest-dir-resolve an optional path field.
pub(crate) fn resolve_opt(base: &Path, p: Option<&PathBuf>) -> Result<Option<PathBuf>, CoreError> {
    match p {
        None => Ok(None),
        Some(p) => Ok(Some(resolve_relative(base, &expand_tilde(p)?))),
    }
}

pub(crate) fn resolve_relative(base: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    }
}

pub(crate) fn validate_slug(name: &str) -> Result<(), String> {
    let ok = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !name.starts_with('-')
        && !name.ends_with('-');
    if ok {
        Ok(())
    } else {
        Err(format!(
            "{name:?} must be a slug: lowercase letters, digits, and interior dashes only"
        ))
    }
}

pub(crate) fn dir_has_md(dir: &Path) -> bool {
    std::fs::read_dir(dir).is_ok_and(|entries| {
        entries
            .filter_map(Result::ok)
            .any(|e| e.path().extension().is_some_and(|ext| ext == "md"))
    })
}

pub(crate) fn is_yaml(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == "yaml" || e == "yml")
}

pub(crate) fn is_hidden(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.starts_with('.'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn five_field_cron_is_accepted() {
        assert!(parse_schedule("*/30 8-18 * * 1-5").is_ok());
        assert!(parse_schedule("not a cron").is_err());
    }

    #[test]
    fn slug_rules() {
        assert!(validate_slug("pr-babysitter").is_ok());
        assert!(validate_slug("Bad Name").is_err());
        assert!(validate_slug("-lead").is_err());
        assert!(validate_slug("").is_err());
    }
}
