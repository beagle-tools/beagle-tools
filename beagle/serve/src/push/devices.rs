//! The registered phones: `devices.json` in serve's state directory, mode
//! `0600`, written atomically, and the checks on what a phone may register.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use super::settings::Settings;
use crate::error::{Error, Result};

/// The most phones one Mac keeps.
pub const MAX_DEVICES: usize = 10;

/// Which APNs host a phone's token belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Environment {
    /// Xcode installs: `api.sandbox.push.apple.com`.
    Development,
    /// `TestFlight` and App Store builds: `api.push.apple.com`.
    Production,
}

/// One phone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Device {
    /// Its APNs device token (secret-ish: never logged). `None` once APNs
    /// rejected it, until the app registers again.
    pub apns_token: Option<String>,
    /// The APNs environment.
    pub environment: Environment,
    /// The app's bundle ID (`apns-topic`).
    pub topic: String,
    /// The phone's name, for `beagle serve status`.
    pub name: String,
    /// First registered, RFC 3339.
    pub registered: String,
    /// Last registered (the app does it on every launch), RFC 3339.
    pub last_seen: String,
    /// Which events it wants.
    pub settings: Settings,
}

/// `devices.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Devices {
    /// By install id (a lowercase UUID the app makes once).
    #[serde(default)]
    pub devices: BTreeMap<String, Device>,
}

/// What `PUT /v1/devices/{id}` takes.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Registration {
    /// The APNs device token, hex.
    pub apns_token: String,
    /// The APNs environment.
    pub environment: Environment,
    /// The app's bundle ID.
    pub topic: String,
    /// The phone's name.
    pub name: String,
}

impl Registration {
    /// Checks every field, for a `400`.
    ///
    /// # Errors
    /// What is wrong, for a person.
    pub fn validate(&self) -> Result<(), String> {
        let token_ok = (64..=200).contains(&self.apns_token.len())
            && self.apns_token.bytes().all(|b| b.is_ascii_hexdigit());
        if !token_ok {
            return Err("apns_token must be 64–200 hex characters".into());
        }
        if !valid_topic(&self.topic) {
            return Err("topic must be a bundle ID like dev.beagletools.beagle".into());
        }
        let name_ok = !self.name.trim().is_empty()
            && self.name.chars().count() <= 64
            && !self.name.chars().any(char::is_control);
        if !name_ok {
            return Err("name must be 1–64 printable characters".into());
        }
        Ok(())
    }
}

/// A reverse-DNS bundle ID of at most 155 characters.
fn valid_topic(topic: &str) -> bool {
    let parts: Vec<&str> = topic.split('.').collect();
    topic.len() <= 155
        && parts.len() >= 2
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'))
}

/// A device id from a path: a UUID, returned lowercase.
#[must_use]
pub fn device_id(raw: &str) -> Option<String> {
    let groups: Vec<&str> = raw.split('-').collect();
    let shape = groups.iter().map(|g| g.len()).collect::<Vec<_>>() == [8, 4, 4, 4, 12];
    (shape
        && groups
            .iter()
            .all(|g| g.bytes().all(|b| b.is_ascii_hexdigit())))
    .then(|| raw.to_ascii_lowercase())
}

/// `devices.json`, read and written under one lock so concurrent requests
/// never lose an update.
#[derive(Debug)]
pub struct DeviceStore {
    path: PathBuf,
    lock: Mutex<()>,
}

impl DeviceStore {
    /// The store at `path` (created on first write).
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            lock: Mutex::new(()),
        }
    }

    /// The file.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The phones now.
    ///
    /// # Errors
    /// [`Error::Io`] or [`Error::Config`] when the file can't be read.
    pub fn load(&self) -> Result<Devices> {
        let _guard = self
            .lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        read(&self.path)
    }

    /// Changes the phones with `f` and saves them; returns what `f` did.
    ///
    /// # Errors
    /// As [`DeviceStore::load`], or the write's error.
    pub fn update<R>(&self, f: impl FnOnce(&mut Devices) -> R) -> Result<R> {
        let _guard = self
            .lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut devices = read(&self.path)?;
        let out = f(&mut devices);
        write(&self.path, &devices)?;
        Ok(out)
    }
}

fn read(path: &Path) -> Result<Devices> {
    let Some(text) = beagle_utils::fs::read_optional(path).map_err(Error::io(path))? else {
        return Ok(Devices::default());
    };
    serde_json::from_str(&text).map_err(|e| Error::Config {
        path: path.to_owned(),
        message: e.to_string(),
    })
}

fn write(path: &Path, devices: &Devices) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(Error::io(parent))?;
    }
    let text = serde_json::to_string_pretty(devices).map_err(|e| Error::Config {
        path: path.to_owned(),
        message: e.to_string(),
    })?;
    beagle_utils::fs::write_atomic_with_mode(path, text.as_bytes(), 0o600).map_err(Error::io(path))
}

/// Forgets every phone (`beagle serve pair --rotate`). A missing file is
/// fine.
///
/// # Errors
/// The removal's error.
pub fn clear(path: &Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(Error::io(path)(e)),
        _ => Ok(()),
    }
}
