//! Errors this crate returns. Apps convert them into their own error enums
//! (`impl From<ToolError> for app::Error`), so these stay small.

use std::io;
use std::path::PathBuf;

/// An external tool (opener, clipboard) failed or was missing.
#[derive(Debug, thiserror::Error)]
#[error("{tool}: {message}")]
pub struct ToolError {
    /// The tool that failed (`pbcopy`, `open`, …).
    pub tool: &'static str,
    /// What went wrong, ready to show in a status line.
    pub message: String,
}

/// A config file could not be read or parsed.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// The file exists but could not be read.
    #[error("i/o error at `{path}`: {source}")]
    Io {
        /// The config file.
        path: PathBuf,
        /// The underlying OS error.
        #[source]
        source: io::Error,
    },
    /// The file is not valid TOML for the app's config schema.
    #[error("failed to parse `{path}`: {source}")]
    Parse {
        /// The config file.
        path: PathBuf,
        /// The TOML deserialization error.
        #[source]
        source: Box<toml::de::Error>,
    },
}
