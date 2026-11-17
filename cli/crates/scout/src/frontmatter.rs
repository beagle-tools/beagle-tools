//! TOML frontmatter between `+++` fences at the top of a Markdown file.
//!
//! ```text
//! +++
//! title = "Map the request path"
//! status = "todo"
//! created = "2026-11-12T15:00:00Z"
//! +++
//!
//! ## Context
//! ...
//! ```
//!
//! TOML rather than YAML: it is the format the rest of the toolchain
//! (Cargo, beagle's `rca.toml`) already uses, and it needs no extra parser.

use std::path::Path;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::error::{Error, Result};

const FENCE: &str = "+++";

/// Splits `source` into `(frontmatter, body)`, or `None` when the file does
/// not open with a `+++` fence that is later closed.
#[must_use]
pub fn split(source: &str) -> Option<(&str, &str)> {
    let rest = source
        .strip_prefix("+++\n")
        .or_else(|| source.strip_prefix("+++\r\n"))?;
    let mut offset = 0;
    for line in rest.split_inclusive('\n') {
        if line.trim_end_matches(['\n', '\r']) == FENCE {
            return Some((&rest[..offset], &rest[offset + line.len()..]));
        }
        offset += line.len();
    }
    None
}

/// The Markdown body of `source`: everything after the frontmatter, or the
/// whole file when there is none.
#[must_use]
pub fn body(source: &str) -> &str {
    split(source).map_or(source, |(_, body)| body)
}

/// Parses `source` (read from `path`) into its typed frontmatter and body.
///
/// # Errors
/// [`Error::Frontmatter`] when the fences are missing or the TOML does not
/// match `T`.
pub fn parse<T: DeserializeOwned>(path: &Path, source: &str) -> Result<(T, String)> {
    let (front, body) = split(source).ok_or_else(|| Error::Frontmatter {
        path: path.to_owned(),
        message: "missing `+++` frontmatter fences at the top of the file".to_owned(),
    })?;
    let meta = toml::from_str(front).map_err(|e| Error::Frontmatter {
        path: path.to_owned(),
        message: e.message().to_owned(),
    })?;
    Ok((meta, body.to_owned()))
}

/// Renders `meta` and `body` back into one file.
///
/// # Errors
/// [`Error::Frontmatter`] if `meta` cannot be expressed as TOML.
pub fn render<T: Serialize>(path: &Path, meta: &T, body: &str) -> Result<String> {
    let front = toml::to_string(meta).map_err(|e| Error::Frontmatter {
        path: path.to_owned(),
        message: e.to_string(),
    })?;
    Ok(format!("{FENCE}\n{front}{FENCE}\n{body}"))
}

#[cfg(test)]
// Tests fail loudly on setup errors; unwrap/expect/panic is the clearest way.
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::model::{TaskMeta, TaskStatus};

    const FILE: &str =
        "+++\ntitle = \"T\"\nstatus = \"todo\"\ncreated = \"2026-11-12T15:00:00Z\"\n+++\n\n## Context\nhi\n";

    #[test]
    fn splits_front_and_body() {
        let (front, body) = split(FILE).unwrap();
        assert!(front.starts_with("title"));
        assert_eq!(body, "\n## Context\nhi\n");
        assert_eq!(super::body(FILE), body);
        assert_eq!(super::body("no fences"), "no fences");
    }

    #[test]
    fn unclosed_or_missing_fence_is_none() {
        assert!(split("+++\ntitle = 1\n").is_none());
        assert!(split("title = 1\n+++\n").is_none());
    }

    #[test]
    fn parse_render_round_trip() {
        let path = Path::new("t.md");
        let (meta, body): (TaskMeta, String) = parse(path, FILE).unwrap();
        assert_eq!(meta.status, TaskStatus::Todo);
        let rendered = render(path, &meta, &body).unwrap();
        let (again, body_again): (TaskMeta, String) = parse(path, &rendered).unwrap();
        assert_eq!(meta, again);
        assert_eq!(body, body_again);
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let bad = FILE.replace("status = \"todo\"", "status = \"todo\"\nowner = \"me\"");
        let err = parse::<TaskMeta>(Path::new("t.md"), &bad).unwrap_err();
        assert!(err.to_string().contains("owner"), "{err}");
    }

    #[test]
    fn crlf_fences_parse() {
        let crlf = FILE.replace('\n', "\r\n");
        assert!(parse::<TaskMeta>(Path::new("t.md"), &crlf).is_ok());
    }
}
