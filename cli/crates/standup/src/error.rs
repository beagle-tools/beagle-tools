//! The crate-wide error type.

use std::io;
use std::path::PathBuf;

/// Every way this crate can fail.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// An I/O operation on a specific file or directory failed.
    #[error("i/o error at `{path}`: {source}")]
    Io {
        /// The file or directory being touched when the error occurred.
        path: PathBuf,
        /// The underlying OS error.
        #[source]
        source: io::Error,
    },

    /// Terminal setup, drawing, or event polling failed.
    #[error("terminal error: {0}")]
    Terminal(#[from] io::Error),

    /// A report date was not a valid `YYYY-MM-DD` calendar date.
    #[error("invalid date `{0}`: expected YYYY-MM-DD")]
    InvalidDate(String),

    /// A ticket id contained characters outside `[A-Za-z0-9_-]`.
    #[error(
        "invalid ticket id `{0}`: use letters, digits, `-` or `_` (e.g. ABC-123), 1..=32 chars"
    )]
    InvalidTicketId(String),

    /// A TOML file (manifest or config) failed to parse.
    #[error("failed to parse `{path}`: {source}")]
    ParseToml {
        /// Path of the offending file.
        path: PathBuf,
        /// The TOML deserialization error.
        #[source]
        source: Box<toml::de::Error>,
    },

    /// A manifest failed to serialize (only on programmer error).
    #[error("failed to serialize manifest: {0}")]
    SerializeToml(#[from] toml::ser::Error),

    /// `--json` output failed to serialize (only on programmer error).
    #[error("failed to serialize JSON output: {0}")]
    SerializeJson(#[from] serde_json::Error),

    /// Scaffolding refused to overwrite something that already exists.
    #[error("`{0}` already exists")]
    AlreadyExists(String),

    /// A report that was asked for does not exist.
    #[error("no report for `{0}` (see `beagle standup list`)")]
    NoSuchReport(String),

    /// The store has no reports at all.
    #[error("no reports yet — create one with `beagle standup new`")]
    NoReports,

    /// A file exceeded the size cap.
    #[error("`{path}` is {size} bytes, over the {limit}-byte limit; refusing to load")]
    FileTooLarge {
        /// Path of the oversized file.
        path: PathBuf,
        /// Actual size in bytes.
        size: u64,
        /// The enforced limit in bytes.
        limit: u64,
    },

    /// The config file could not be read or parsed.
    #[error(transparent)]
    Config(#[from] beagle_tui::ConfigError),

    /// The filesystem watcher could not be created or attached.
    #[error("filesystem watcher error: {0}")]
    Watch(#[from] notify::Error),

    /// An external tool (opener, clipboard) failed or was missing.
    #[error("{tool}: {message}")]
    Tool {
        /// The tool that failed.
        tool: &'static str,
        /// What went wrong.
        message: String,
    },
}

impl Error {
    /// Wraps an [`io::Error`] with the path being operated on.
    pub fn io(path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}

impl From<beagle_tui::ToolError> for Error {
    fn from(e: beagle_tui::ToolError) -> Self {
        Self::Tool {
            tool: e.tool,
            message: e.message,
        }
    }
}

/// Crate-wide result alias.
pub type Result<T> = std::result::Result<T, Error>;
