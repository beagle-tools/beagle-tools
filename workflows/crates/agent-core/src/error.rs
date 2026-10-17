use std::path::PathBuf;

/// Unified error type for the core crate.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    /// A filesystem operation failed.
    #[error("io error at {path}: {source}")]
    Io {
        /// The file or directory the operation was on.
        path: PathBuf,
        /// The underlying OS error.
        #[source]
        source: std::io::Error,
    },

    /// The `SQLite` store rejected a query or could not be opened.
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),

    /// A JSON column (spec, params, usage) failed to (de)serialise.
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    /// A manifest file is unreadable or invalid.
    #[error("manifest error in {path}: {message}")]
    Manifest {
        /// The offending manifest file.
        path: PathBuf,
        /// What is wrong with it, for a human.
        message: String,
    },

    /// A `schedule:` is not a valid 5-field cron expression.
    #[error("invalid cron schedule {expr:?}: {message}")]
    Schedule {
        /// The expression as written in the manifest.
        expr: String,
        /// The cron parser's complaint.
        message: String,
    },

    /// A stored or user-supplied string names no variant of an enum.
    #[error("unknown {kind} value: {value:?}")]
    UnknownVariant {
        /// Human name of the enum (`runner`, `run state`, ...).
        kind: &'static str,
        /// The string that matched nothing.
        value: String,
    },

    /// Anything else; the string is the whole message.
    #[error("{0}")]
    Other(String),
}

impl CoreError {
    /// Shorthand for [`CoreError::Io`].
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }

    /// Shorthand for [`CoreError::Manifest`].
    pub fn manifest(path: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Self::Manifest {
            path: path.into(),
            message: message.into(),
        }
    }
}
