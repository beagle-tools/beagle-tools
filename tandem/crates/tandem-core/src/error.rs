//! The workspace-wide error type, [`TandemError`].

use thiserror::Error;

/// Unified error type for the Tandem workspace.
///
/// Crates map their internal failures into these variants so the Tauri
/// layer can serialize one shape to the frontend.
#[derive(Debug, Error)]
pub enum TandemError {
    /// GitHub answered with a non-success status.
    #[error("GitHub API error ({status}): {message}")]
    GithubApi {
        /// HTTP status code GitHub returned.
        status: u16,
        /// GitHub's error message (or the raw body when it had none).
        message: String,
    },

    /// No usable GitHub credentials (missing token, `gh` logged out, ...).
    #[error("authentication error: {0}")]
    Auth(String),

    /// The SQLite cache failed (open, query, or migration).
    #[error("cache error: {0}")]
    Cache(String),

    /// An agent could not be configured, spawned, or managed.
    #[error("agent runner error: {0}")]
    Agent(String),

    /// A unified diff could not be parsed.
    #[error("diff parse error: {0}")]
    DiffParse(String),

    /// Invalid or missing settings / user configuration.
    #[error("configuration error: {0}")]
    Config(String),

    /// Filesystem or process I/O failed.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON (de)serialization failed.
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

impl serde::Serialize for TandemError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}
