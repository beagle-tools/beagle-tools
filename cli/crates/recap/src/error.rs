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

    /// A recap id contained characters outside `[a-z0-9-]`.
    #[error("invalid recap id `{0}`: use lowercase letters, digits and `-` (e.g. widgets-42)")]
    InvalidId(String),

    /// A pull request reference couldn't be understood.
    #[error(
        "can't read PR `{0}`: use a URL (https://github.com/<owner>/<repo>/pull/<n>) or <owner>/<repo>#<n>"
    )]
    InvalidPr(String),

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

    /// JSON failed to (de)serialize.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Scaffolding refused to overwrite something that already exists.
    #[error("`{0}` already exists (use `beagle recap refresh` to re-pull it)")]
    AlreadyExists(String),

    /// A recap that was asked for does not exist.
    #[error("no recap `{0}` (see `beagle recap list`)")]
    NoSuchRecap(String),

    /// The store has no recaps at all.
    #[error("no recaps yet — create one with `beagle recap new <PR url>`")]
    NoRecaps,

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

    /// An external tool (gh, opener, clipboard) failed or was missing.
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
