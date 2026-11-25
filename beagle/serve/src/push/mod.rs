//! Push notifications, Mac side (`docs/serve-push.md` is the contract):
//! the registered phones and their settings ([`devices`], [`settings`]),
//! following each tool's event spool ([`follow`]), and matching events to
//! phones ([`route`]) for a [`Sender`].
//!
//! ```text
//! events/<tool>.jsonl ──▶ Follower::poll ──▶ route(event, devices) ──▶ Sender
//!                                               ▲
//! PUT /v1/devices/… ──▶ DeviceStore (devices.json, 0600)
//! ```
//!
//! One background thread runs the loop every [`POLL_EVERY`]. Until APNs
//! is configured the sender is [`HeldSender`], which only logs.

pub mod devices;
pub mod follow;
pub mod settings;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use beagle_scout::events::Record;
use time::OffsetDateTime;

use crate::error::{Error, Result};
use devices::{DeviceStore, Devices, Environment};
use follow::Follower;

/// How often the spools are read.
pub const POLL_EVERY: Duration = Duration::from_secs(2);

/// `$XDG_STATE_HOME/beagle/serve` (or `~/.local/state/beagle/serve`):
/// `devices.json` and the spool cursor live here.
///
/// # Errors
/// [`Error::NoConfigDir`] without a home directory.
pub fn state_dir() -> Result<PathBuf> {
    crate::service::log_dir()
}

/// `devices.json`'s path.
///
/// # Errors
/// As [`state_dir`].
pub fn devices_path() -> Result<PathBuf> {
    Ok(state_dir()?.join("devices.json"))
}

/// One notification to send: an event, for one phone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pending {
    /// The phone's install id.
    pub device: String,
    /// Its APNs token.
    pub apns_token: String,
    /// Its APNs environment.
    pub environment: Environment,
    /// Its app's bundle ID.
    pub topic: String,
    /// The event.
    pub record: Record,
}

/// The phones that want `record`: registered (with a token) and with
/// settings that allow its tool and kind.
#[must_use]
pub fn route(record: &Record, devices: &Devices) -> Vec<Pending> {
    devices
        .devices
        .iter()
        .filter(|(_, d)| settings::allows(&d.settings, &record.tool, &record.kind))
        .filter_map(|(id, d)| {
            Some(Pending {
                device: id.clone(),
                apns_token: d.apns_token.clone()?,
                environment: d.environment,
                topic: d.topic.clone(),
                record: record.clone(),
            })
        })
        .collect()
}

/// Delivers notifications.
pub trait Sender: Send {
    /// Sends one; failures are the sender's to log and handle.
    fn send(&mut self, pending: &Pending);
}

/// The sender while push isn't configured: logs what it would send.
#[derive(Debug, Default)]
pub struct HeldSender;

impl Sender for HeldSender {
    fn send(&mut self, p: &Pending) {
        eprintln!(
            "beagle serve: push: held {} {} for device {}: push is not configured ([push] in serve.toml)",
            p.record.tool, p.record.kind, p.device
        );
    }
}

/// One round: new events, matched against the phones, handed to `sender`.
/// Returns how many notifications went to the sender.
pub fn tick(follower: &mut Follower, store: &DeviceStore, sender: &mut dyn Sender) -> usize {
    let (records, problems) = follower.poll(OffsetDateTime::now_utc());
    for problem in problems {
        eprintln!("beagle serve: push: {problem}");
    }
    if records.is_empty() {
        return 0;
    }
    let devices = match store.load() {
        Ok(devices) => devices,
        Err(e) => {
            eprintln!("beagle serve: push: {e}");
            return 0;
        }
    };
    let mut sent = 0;
    for record in &records {
        for pending in route(record, &devices) {
            sender.send(&pending);
            sent += 1;
        }
    }
    sent
}

/// Starts the background loop: follows every tool's spool and hands
/// matches to `sender`.
///
/// # Errors
/// [`Error::NoConfigDir`] without a home directory, or the thread's error.
pub fn spawn(store: Arc<DeviceStore>, mut sender: Box<dyn Sender>) -> Result<()> {
    let events = beagle_scout::events::dir().ok_or(Error::NoConfigDir)?;
    let cursor = state_dir()?.join("push-cursor.json");
    let tools = settings::CATALOG
        .iter()
        .map(|t| t.tool.to_owned())
        .collect();
    let mut follower = Follower::new(events, cursor, tools);
    std::thread::Builder::new()
        .name("serve-push".into())
        .spawn(move || loop {
            tick(&mut follower, &store, sender.as_mut());
            std::thread::sleep(POLL_EVERY);
        })
        .map(|_| ())
        .map_err(|e| Error::Service(format!("could not start the push thread: {e}")))
}

#[cfg(test)]
#[path = "../tests/push.rs"]
mod tests;
