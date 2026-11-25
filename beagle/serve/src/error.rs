//! The crate's single error type.

use std::net::SocketAddr;
use std::path::PathBuf;

/// Everything that can stop `beagle serve`, with enough context to act on.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A filesystem operation failed.
    #[error("{}: {source}", .path.display())]
    Io {
        /// The file or directory involved.
        path: PathBuf,
        /// The underlying error.
        #[source]
        source: std::io::Error,
    },
    /// `serve.toml` exists but is not valid.
    #[error("{}: {message}", .path.display())]
    Config {
        /// The config file.
        path: PathBuf,
        /// What is wrong with it.
        message: String,
    },
    /// No home or config directory to keep `serve.toml` in.
    #[error("no config directory: set $HOME or $XDG_CONFIG_HOME")]
    NoConfigDir,
    /// The command line was wrong.
    #[error("{0}\nrun `beagle serve --help` for usage")]
    Usage(String),
    /// There is no token yet.
    #[error("no token yet: run `beagle serve pair` to create one")]
    NoToken,
    /// The token in `serve.toml` is malformed.
    #[error("the token in {} is not 64 hex characters: run `beagle serve pair --rotate`", .0.display())]
    BadToken(PathBuf),
    /// The TLS certificate or key is missing.
    #[error(
        "no TLS certificate at {} (or its key at {}). Create both with:\n\n  mkdir -p {}\n  tailscale cert --cert-file {} --key-file {} {host}\n\n(HTTPS certificates must be enabled for your tailnet: admin console → DNS → HTTPS Certificates)",
        .cert.display(), .key.display(), .dir.display(), .cert.display(), .key.display()
    )]
    MissingCert {
        /// Where the certificate was expected.
        cert: PathBuf,
        /// Where the key was expected.
        key: PathBuf,
        /// Their directory.
        dir: PathBuf,
        /// The `MagicDNS` name the certificate is for.
        host: String,
    },
    /// The certificate or key could not be loaded.
    #[error("TLS: {0}")]
    Tls(String),
    /// Asking Tailscale for this machine's address or name failed.
    #[error("tailscale: {0} — is Tailscale running and signed in? (or pass --addr / set host in serve.toml)")]
    Tailscale(String),
    /// The address is one the server refuses to bind without a flag.
    #[error("{0}")]
    Refused(String),
    /// Installing, removing or asking about the launchd agent failed.
    #[error("service: {0}")]
    Service(String),
    /// Push notifications: not set up, no phones, or nothing delivered.
    #[error("push: {0}")]
    Push(String),
    /// Listening failed.
    #[error("can't listen on {addr}: {source}")]
    Bind {
        /// The address.
        addr: SocketAddr,
        /// The underlying error.
        #[source]
        source: std::io::Error,
    },
}

impl Error {
    /// A closure wrapping an [`std::io::Error`] with the path it happened on.
    pub fn io(path: impl Into<PathBuf>) -> impl FnOnce(std::io::Error) -> Self {
        let path = path.into();
        move |source| Self::Io { path, source }
    }
}

/// `Result` with [`Error`].
pub type Result<T, E = Error> = std::result::Result<T, E>;
