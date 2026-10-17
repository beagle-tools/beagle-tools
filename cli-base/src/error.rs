//! The crate-wide error type.
//!
//! One closed enum, so callers can match on failure modes, every I/O failure
//! carries the path it happened on, and every message tells the user what to
//! do next.

use std::io;
use std::path::{Path, PathBuf};

/// Every way the base CLI can fail.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The command line did not parse. Exits 2, like rca.
    #[error("{0}")]
    Usage(String),

    /// `beagle <name>` matched no base command, extension or `PATH` binary.
    #[error("unknown command \"{name}\"{hint}")]
    UnknownCommand {
        /// What the user typed.
        name: String,
        /// A ready-made `— did you mean …?` suffix, or empty.
        hint: String,
    },

    /// An extension name is not in the registry.
    #[error("unknown extension \"{name}\" — run: beagle extensions available")]
    UnknownExtension {
        /// What the user typed.
        name: String,
    },

    /// A base command name was given where an extension name belongs.
    #[error("\"{name}\" is a reserved command name, not an extension")]
    Reserved {
        /// The reserved name.
        name: String,
    },

    /// A registered extension is not installed.
    #[error("\"{name}\" is not installed — run: beagle extensions install {name}")]
    NotInstalled {
        /// The extension.
        name: String,
    },

    /// A version string is not `X.Y.Z` (optionally `v`/tag-prefixed).
    #[error("invalid version `{input}` (expected X.Y.Z, vX.Y.Z or {prefix}X.Y.Z)")]
    InvalidVersion {
        /// What the user typed.
        input: String,
        /// The tag prefix that would have been accepted.
        prefix: String,
    },

    /// The requested version was never released (for this target).
    #[error("{name} {version} is not released for {target}{nearest}")]
    NoSuchVersion {
        /// The extension (or `beagle` for the base).
        name: String,
        /// The requested version.
        version: String,
        /// The release target.
        target: String,
        /// A ready-made `; nearest: …` suffix, or empty.
        nearest: String,
    },

    /// `extensions use` named a version that is not on disk.
    #[error("{name} {version} is not installed — run: beagle extensions install {name} --version {version}")]
    VersionNotInstalled {
        /// The extension.
        name: String,
        /// The requested version.
        version: String,
    },

    /// No release of the extension carries a build for this platform.
    #[error("{name} is not available for this platform ({target})")]
    UnsupportedTarget {
        /// The extension (or `beagle`).
        name: String,
        /// This build's target triple, or a description of the platform.
        target: String,
    },

    /// The extension supports this platform, but no release carries a
    /// build for it yet.
    #[error("no {name} release has a build for {target} yet — see: beagle extensions available")]
    NotReleased {
        /// The extension (or `beagle`).
        name: String,
        /// This build's target triple.
        target: String,
    },

    /// An I/O operation on a specific file or directory failed.
    #[error("i/o error at `{path}`: {source}")]
    Io {
        /// The file or directory being touched.
        path: PathBuf,
        /// The underlying OS error.
        #[source]
        source: io::Error,
    },

    /// An external tool (curl, tar, shasum) failed, was missing, or timed out.
    #[error("{tool} failed: {message}")]
    Tool {
        /// The tool that failed.
        tool: &'static str,
        /// Its stderr, or why it could not run.
        message: String,
    },

    /// A downloaded tarball does not match its published sha256.
    #[error("checksum mismatch for {asset}: expected {expected}, got {actual}")]
    ChecksumMismatch {
        /// The asset file name.
        asset: String,
        /// The published digest.
        expected: String,
        /// The digest of what was downloaded.
        actual: String,
    },

    /// The GitHub releases listing was not the expected JSON.
    #[error("could not read the release list: {0}")]
    ParseReleases(String),

    /// An `extension.toml` (or a state file) failed to parse or is invalid.
    #[error("invalid `{path}`: {message}")]
    Manifest {
        /// The offending file.
        path: PathBuf,
        /// What is wrong with it.
        message: String,
    },

    /// A tarball declares a different extension than the one requested.
    #[error("tarball for \"{expected}\" declares name \"{found}\"")]
    NameMismatch {
        /// The extension being installed.
        expected: String,
        /// The name in its `extension.toml`.
        found: String,
    },

    /// An extension needs a newer dispatch protocol than this base speaks.
    #[error("{ext} {version} needs a newer beagle (protocol {protocol}) — run: beagle update")]
    ProtocolTooNew {
        /// The extension.
        ext: String,
        /// Its version.
        version: String,
        /// The protocol it declares.
        protocol: u32,
    },

    /// An extension declares a `min_base_version` above this base.
    #[error(
        "{ext} {version} needs beagle {required} or newer (this is {current}) — run: beagle update"
    )]
    BaseTooOld {
        /// The extension.
        ext: String,
        /// Its version.
        version: String,
        /// The base version it requires.
        required: String,
        /// This base's version.
        current: String,
    },

    /// Another `beagle` process holds the install lock.
    #[error("another beagle is installing {name} — try again when it finishes")]
    Locked {
        /// The extension (or `beagle`).
        name: String,
    },

    /// Executing the extension binary failed.
    #[error("could not run {}: {source} — try: beagle extensions install {name}", path.display())]
    Exec {
        /// The binary that could not be executed.
        path: PathBuf,
        /// The extension it belongs to.
        name: String,
        /// Why exec failed.
        #[source]
        source: io::Error,
    },

    /// `beagle update` on a Homebrew-managed install.
    #[error("installed by Homebrew — run: brew upgrade beagle")]
    ManagedByHomebrew,

    /// The interactive picker's terminal I/O failed.
    #[error("terminal error: {0}")]
    Terminal(#[source] io::Error),

    /// `$XDG_DATA_HOME` and `$HOME` are both unset.
    #[error("cannot find a data directory: set $HOME or $XDG_DATA_HOME")]
    NoDataDir,
}

impl Error {
    /// Wraps an I/O error with the path it happened on.
    pub fn io(path: impl AsRef<Path>, source: io::Error) -> Self {
        Self::Io {
            path: path.as_ref().to_path_buf(),
            source,
        }
    }

    /// The process exit code for this error: 2 for "you typed it wrong"
    /// (usage, unknown command), 1 for everything else.
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Usage(_) | Self::UnknownCommand { .. } => 2,
            _ => 1,
        }
    }
}

/// `Result` with this crate's [`Error`].
pub type Result<T, E = Error> = std::result::Result<T, E>;
