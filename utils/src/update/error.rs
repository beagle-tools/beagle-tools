//! Why an update failed. Apps convert this into their own error enum.

use std::fmt;
use std::io;
use std::path::PathBuf;

/// Every way listing or installing a release can fail.
#[derive(Debug)]
pub enum Error {
    /// An external tool (`curl`, `tar`, `shasum`) failed or was missing.
    Tool {
        /// The tool that failed.
        tool: &'static str,
        /// What went wrong, including any stderr worth showing.
        message: String,
    },
    /// The GitHub releases feed could not be understood.
    ParseReleases(String),
    /// A downloaded tarball's checksum did not match the published one.
    ChecksumMismatch {
        /// The downloaded file.
        path: PathBuf,
        /// The checksum published beside the release asset.
        expected: String,
        /// The checksum computed from the download.
        actual: String,
    },
    /// No prebuilt release binary exists for this platform.
    UnsupportedTarget,
    /// A file operation during the install failed.
    Io {
        /// The file or directory being touched.
        path: PathBuf,
        /// The underlying OS error.
        source: io::Error,
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

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tool { tool, message } => write!(f, "{tool}: {message}"),
            Self::ParseReleases(message) => {
                write!(f, "could not parse the releases feed: {message}")
            }
            Self::ChecksumMismatch {
                path,
                expected,
                actual,
            } => write!(
                f,
                "checksum mismatch for `{}`: expected {expected}, got {actual}",
                path.display()
            ),
            Self::UnsupportedTarget => {
                write!(f, "no prebuilt binary for {}", super::TARGET_DESCRIPTION)
            }
            Self::Io { path, source } => {
                write!(f, "i/o error at `{}`: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Tool { .. }
            | Self::ParseReleases(_)
            | Self::ChecksumMismatch { .. }
            | Self::UnsupportedTarget => None,
        }
    }
}
