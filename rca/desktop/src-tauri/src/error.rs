//! The one error type every Tauri command returns.
//!
//! The frontend contract is that a rejected `invoke` carries a plain,
//! actionable message string. [`Error`] keeps the failure modes typed on the
//! Rust side and serializes to exactly its `Display` text, so the JSON the
//! webview sees is still just that string.

use std::io;
use std::path::PathBuf;

use serde::{Serialize, Serializer};

/// Every way a desktop command can fail. The `Display` text is what the user
/// sees, so each variant's message says what to do next where it can.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The domain layer (store, ids, config) failed; its message already
    /// names the file or value involved.
    #[error(transparent)]
    Beagle(#[from] beagle::Error),

    /// Neither the config file nor the working directory gave a usable
    /// workspace root.
    #[error("cannot resolve a workspace root: {0}")]
    NoRoot(#[source] io::Error),

    /// The frontend asked for a section file name that no [`SectionKind`]
    /// owns.
    ///
    /// [`SectionKind`]: beagle::model::SectionKind
    #[error("unknown section `{0}`")]
    UnknownSection(String),

    /// A blocking task spawned off the main thread panicked or was
    /// cancelled.
    #[error("{0}")]
    Task(#[from] tauri::Error),

    /// The `beagle-agentd` control socket could not be connected to.
    /// `hint` adds the "how to start it" suffix for one-shot requests; the
    /// live status stream omits it.
    #[error(
        "daemon not reachable at {} ({source}){}",
        path.display(),
        if *hint { "; is it running? try `beagle agent start`" } else { "" }
    )]
    DaemonUnreachable {
        /// The socket path that was tried.
        path: PathBuf,
        /// The connect error.
        #[source]
        source: io::Error,
        /// Whether to append the start-the-daemon hint.
        hint: bool,
    },

    /// An I/O step on an open control connection failed; `op` is the step
    /// (`error` for cloning the stream, else `write`, `flush`, `read`).
    #[error("socket {op}: {source}")]
    Socket {
        /// The failing step, used verbatim in the message.
        op: &'static str,
        /// The underlying I/O error.
        #[source]
        source: io::Error,
    },

    /// A response line was not valid JSON.
    #[error("bad daemon response: {0}")]
    BadResponse(#[source] serde_json::Error),

    /// A `status`/`update` response had a `status` field of the wrong shape.
    #[error("bad status payload: {0}")]
    BadStatus(#[source] serde_json::Error),

    /// A `status`/`update` response had no `status` field at all.
    #[error("daemon response missing status")]
    MissingStatus,

    /// The daemon answered with an `error` response; this is its message.
    #[error("{0}")]
    Daemon(String),

    /// The daemon answered with a response type this request never expects.
    #[error("unexpected daemon response")]
    UnexpectedResponse,
}

impl Serialize for Error {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

/// Shorthand for command results.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
#[path = "tests/error.rs"]
mod tests;
