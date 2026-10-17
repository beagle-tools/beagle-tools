//! `extension.toml`: the manifest shipped at the root of every extension
//! tarball. A public format (`CODING_STANDARDS.md` §1.13) — see `AGENTS.md`.

use std::path::Path;

use serde::Deserialize;

use crate::error::{Error, Result};
use crate::version::Version;

/// The only manifest schema this base reads.
pub const SCHEMA: u32 = 1;

/// The newest dispatch protocol (env contract, exec semantics) this base
/// speaks. Extensions declaring a higher one are refused.
pub const PROTOCOL: u32 = 1;

/// A parsed, validated `extension.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtensionManifest {
    /// Manifest format version; always [`SCHEMA`] once parsed.
    pub schema: u32,
    /// Canonical extension name; must match the registry entry.
    pub name: String,
    /// The extension's version.
    pub version: Version,
    /// Dispatch-contract version it was built for.
    pub protocol: u32,
    /// Oldest base that can run it.
    pub min_base_version: Version,
    /// The entrypoint binary the base execs.
    pub bin: String,
    /// Sibling binaries installed alongside, never dispatched to directly.
    #[serde(default)]
    pub extra_bins: Vec<String>,
    /// One-line description for help output.
    pub description: String,
    /// Arguments the base passes to `bin` after an update (e.g.
    /// `["daemon", "restart"]`); empty means no hook.
    #[serde(default)]
    pub post_update: Vec<String>,
}

impl ExtensionManifest {
    /// Parses and validates manifest text. `path` is only used in errors.
    pub fn parse(text: &str, path: &Path) -> Result<Self> {
        let bad = |message: String| Error::Manifest {
            path: path.to_path_buf(),
            message,
        };
        // Check the schema first so a future format gets an upgrade hint
        // rather than a confusing unknown-field error.
        let schema = toml::from_str::<toml::Table>(text)
            .map_err(|e| bad(e.message().to_owned()))?
            .get("schema")
            .and_then(toml::Value::as_integer);
        match schema {
            Some(s) if s == i64::from(SCHEMA) => {}
            Some(s) => {
                return Err(bad(format!(
                    "manifest schema {s} is not supported (this beagle reads schema {SCHEMA}) — run: beagle update"
                )))
            }
            None => return Err(bad("missing integer `schema`".to_owned())),
        }
        let manifest: Self = toml::from_str(text).map_err(|e| bad(e.message().to_owned()))?;
        for bin in std::iter::once(&manifest.bin).chain(&manifest.extra_bins) {
            if !is_plain_file_name(bin) {
                return Err(bad(format!(
                    "binary name `{bin}` must be a plain file name"
                )));
            }
        }
        Ok(manifest)
    }

    /// Reads and parses the manifest at `path`.
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).map_err(|e| Error::io(path, e))?;
        Self::parse(&text, path)
    }

    /// Fails unless this manifest declares extension `expected`.
    pub fn check_name(&self, expected: &str) -> Result<()> {
        if self.name == expected {
            Ok(())
        } else {
            Err(Error::NameMismatch {
                expected: expected.to_owned(),
                found: self.name.clone(),
            })
        }
    }

    /// Fails when this base (`base`) cannot run the extension.
    pub fn check_compatible(&self, base: Version) -> Result<()> {
        if self.protocol > PROTOCOL {
            return Err(Error::ProtocolTooNew {
                ext: self.name.clone(),
                version: self.version.to_string(),
                protocol: self.protocol,
            });
        }
        if self.min_base_version > base {
            return Err(Error::BaseTooOld {
                ext: self.name.clone(),
                version: self.version.to_string(),
                required: self.min_base_version.to_string(),
                current: base.to_string(),
            });
        }
        Ok(())
    }

    /// Every binary the tarball must contain: `bin`, then `extra_bins`.
    pub fn all_bins(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.bin.as_str()).chain(self.extra_bins.iter().map(String::as_str))
    }
}

/// True for a single, ordinary path component: no separators, no `.`/`..`.
fn is_plain_file_name(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && !name.contains(['/', '\\', '\0'])
}

#[cfg(test)]
#[path = "tests/manifest.rs"]
mod tests;
