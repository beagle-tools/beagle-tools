//! `~/.config/beagle/serve.toml` (`$XDG_CONFIG_HOME/beagle/serve.toml`):
//! the token, and where to listen.
//!
//! ```toml
//! token = "…64 hex characters…"           # written by `beagle serve pair`
//! token_created = 2026-01-01T00:00:00Z     # when pair made it (`beagle doctor` warns when old)
//! host = "mac.example-tailnet.ts.net"     # MagicDNS name, for the cert and the pairing URL
//! port = 8787
//! # addr = "100.64.0.1"                   # default: `tailscale ip -4`
//! # cert = "~/.config/beagle/serve/<host>.crt"
//! # key  = "~/.config/beagle/serve/<host>.key"
//!
//! [push]                                  # push notifications (docs/serve-push.md);
//! team_id = "ABC123DEFG"                  # written by `beagle serve push setup`
//! key_id  = "KEY1234567"
//! key     = "~/.config/beagle/serve/AuthKey_KEY1234567.p8"
//!
//! [secrets]                               # or the key from a secret manager, kept
//! command = "my-vault export --json"      # in the Keychain: `beagle serve secrets sync`
//! ```
//!
//! The file holds a secret, so it is written `0600`, atomically.

use std::fmt;
use std::net::IpAddr;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// The port when none is configured.
pub const DEFAULT_PORT: u16 = 8787;

/// `serve.toml`. Every field is optional: `pair` fills in what it learns.
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// The bearer token (secret).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    /// When `pair` made the token; unknown for tokens made before this was
    /// recorded.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "time::serde::rfc3339::option"
    )]
    pub token_created: Option<time::OffsetDateTime>,
    /// The Mac's `MagicDNS` name (`<mac>.<tailnet>.ts.net`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    /// The port to listen on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    /// The address to bind, overriding the Tailscale address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub addr: Option<IpAddr>,
    /// The certificate (PEM), overriding the default path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cert: Option<PathBuf>,
    /// The private key (PEM), overriding the default path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<PathBuf>,
    /// Push notifications through APNs; `None` means push is off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub push: Option<PushConfig>,
    /// The secret-manager command `beagle serve secrets sync` runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secrets: Option<SecretsConfig>,
}

/// `[secrets]`: where `beagle serve secrets sync` gets the push key
/// (`docs/serve-push.md`, "Keeping the key in a secret manager").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SecretsConfig {
    /// Run with `sh -c`; prints JSON or dotenv holding `BEAGLE_APNS_*`.
    pub command: String,
}

/// `[push]`: the APNs token-auth key from the Apple developer account.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PushConfig {
    /// The developer account's Team ID (10 characters).
    pub team_id: String,
    /// The APNs key's Key ID (10 characters).
    pub key_id: String,
    /// The `.p8` key file (`~` expanded); must be mode `0600`.
    pub key: PathBuf,
}

impl PushConfig {
    /// The key file's path, `~` expanded.
    #[must_use]
    pub fn key_path(&self) -> PathBuf {
        beagle_utils::paths::expand_tilde(&self.key)
    }

    /// Checks the two ids: 10 uppercase letters or digits, as Apple makes
    /// them.
    ///
    /// # Errors
    /// What is wrong, naming the field.
    pub fn validate_ids(&self) -> std::result::Result<(), String> {
        for (name, value) in [("team_id", &self.team_id), ("key_id", &self.key_id)] {
            let ok = value.len() == 10
                && value
                    .bytes()
                    .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit());
            if !ok {
                return Err(format!(
                    "{name} must be 10 uppercase letters or digits, not {value:?}"
                ));
            }
        }
        Ok(())
    }
}

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("token", &self.token.as_ref().map(|_| "<redacted>"))
            .field("token_created", &self.token_created)
            .field("host", &self.host)
            .field("port", &self.port)
            .field("addr", &self.addr)
            .field("cert", &self.cert)
            .field("key", &self.key)
            .field("push", &self.push)
            .field("secrets", &self.secrets)
            .finish()
    }
}

impl Config {
    /// The configured port, or [`DEFAULT_PORT`].
    #[must_use]
    pub fn port(&self) -> u16 {
        self.port.unwrap_or(DEFAULT_PORT)
    }

    /// When the token was made, if known.
    #[must_use]
    pub fn token_created_at(&self) -> Option<std::time::SystemTime> {
        self.token_created.map(std::time::SystemTime::from)
    }

    /// The certificate and key paths for `host`: the configured ones
    /// (`~` expanded), else `<dir>/<host>.crt` and `<dir>/<host>.key`.
    #[must_use]
    pub fn cert_paths(&self, dir: &Path, host: &str) -> (PathBuf, PathBuf) {
        let pick = |configured: &Option<PathBuf>, ext: &str| {
            configured.as_deref().map_or_else(
                || dir.join(format!("{host}.{ext}")),
                beagle_utils::paths::expand_tilde,
            )
        };
        (pick(&self.cert, "crt"), pick(&self.key, "key"))
    }
}

/// `$XDG_CONFIG_HOME/beagle` (or `~/.config/beagle`).
///
/// # Errors
/// [`Error::NoConfigDir`] with neither variable set.
pub fn dir() -> Result<PathBuf> {
    beagle_utils::paths::xdg_config_home()
        .map(|d| d.join("beagle"))
        .ok_or(Error::NoConfigDir)
}

/// The config file's path.
///
/// # Errors
/// As [`dir`].
pub fn path() -> Result<PathBuf> {
    Ok(dir()?.join("serve.toml"))
}

/// Where `tailscale cert` output goes by default: `<config dir>/serve/`.
///
/// # Errors
/// As [`dir`].
pub fn cert_dir() -> Result<PathBuf> {
    Ok(dir()?.join("serve"))
}

/// Loads `path`; a missing file is the default config.
///
/// # Errors
/// [`Error::Io`] when it can't be read, [`Error::Config`] when it does not
/// parse (unknown keys included, so typos fail loudly).
pub fn load(path: &Path) -> Result<Config> {
    let Some(text) = beagle_utils::fs::read_optional(path).map_err(Error::io(path))? else {
        return Ok(Config::default());
    };
    toml::from_str(&text).map_err(|e| Error::Config {
        path: path.to_owned(),
        message: e.message().to_owned(),
    })
}

/// Writes `config` to `path` atomically with mode `0600`, creating the
/// directory.
///
/// # Errors
/// [`Error::Io`] when the directory or file can't be written.
pub fn save(path: &Path, config: &Config) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(Error::io(parent))?;
    }
    let text = toml::to_string(config).map_err(|e| Error::Config {
        path: path.to_owned(),
        message: e.to_string(),
    })?;
    beagle_utils::fs::write_atomic_with_mode(path, text.as_bytes(), 0o600).map_err(Error::io(path))
}

/// Whether anyone but the owner may read `path` (`false` off Unix, or when
/// it can't be read at all).
#[must_use]
pub fn readable_by_others(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::metadata(path).is_ok_and(|m| m.permissions().mode() & 0o077 != 0)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        false
    }
}

/// A warning when `path` can be read by anyone but its owner.
#[must_use]
pub fn permission_warning(path: &Path) -> Option<String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        // An unreadable mode is reported by the load that follows.
        let mode = std::fs::metadata(path).ok()?.permissions().mode();
        (mode & 0o077 != 0).then(|| {
            format!(
                "{} is readable by others (mode {:o}); it holds your token: chmod 600 it",
                path.display(),
                mode & 0o777
            )
        })
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        None
    }
}

#[cfg(test)]
#[path = "tests/config.rs"]
mod tests;
