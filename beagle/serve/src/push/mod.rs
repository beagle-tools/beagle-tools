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
//! One background thread runs the loop every [`POLL_EVERY`]. The sender is
//! [`apns::ApnsSender`] when `[push]` is set in `serve.toml`, else
//! [`HeldSender`], which only logs. `POST /v1/devices/{id}/test` and
//! `beagle serve push test` share it.

pub mod apns;
pub mod cmd;
pub mod devices;
pub mod follow;
pub mod jwt;
pub mod settings;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use beagle_scout::events::Record;
use time::OffsetDateTime;

use crate::config::{self, Config};
use crate::error::{Error, Result};
use devices::{Device, DeviceStore, Devices, Environment};
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

/// How one send went.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// Whether APNs accepted it.
    pub delivered: bool,
    /// What happened, for the log or the person asking.
    pub message: String,
}

impl Outcome {
    /// Delivered.
    #[must_use]
    pub fn sent(message: String) -> Self {
        Self {
            delivered: true,
            message,
        }
    }

    /// Not delivered.
    #[must_use]
    pub fn failed(message: String) -> Self {
        Self {
            delivered: false,
            message,
        }
    }
}

/// Delivers notifications.
pub trait Sender: Send + std::fmt::Debug {
    /// Sends one; the sender handles retries and dead phones itself.
    fn send(&mut self, pending: &Pending) -> Outcome;

    /// Whether it really sends (`false` while push isn't configured).
    fn configured(&self) -> bool {
        true
    }
}

/// The sender, shared by the push thread and the test endpoint.
pub type SharedSender = Arc<Mutex<Box<dyn Sender>>>;

/// The sender while push isn't configured: logs what it would send.
#[derive(Debug, Default)]
pub struct HeldSender;

impl Sender for HeldSender {
    fn send(&mut self, _: &Pending) -> Outcome {
        Outcome::failed("held: push is not configured ([push] in serve.toml)".into())
    }

    fn configured(&self) -> bool {
        false
    }
}

/// The sender `config` asks for: APNs with its key, or [`HeldSender`]
/// without `[push]`. A broken `[push]` is an error, never a crash.
///
/// # Errors
/// What is wrong with `[push]` or its key, for a person.
pub fn sender_for(
    config: &Config,
    store: Arc<DeviceStore>,
) -> std::result::Result<Box<dyn Sender>, String> {
    let Some(push) = &config.push else {
        return Ok(Box::new(HeldSender));
    };
    let signer = cmd::signer(push)?;
    Ok(Box::new(apns::ApnsSender::new(signer, store)?))
}

/// Sends `p` and logs how it went.
fn deliver(sender: &SharedSender, p: &Pending) -> Outcome {
    let outcome = sender
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .send(p);
    eprintln!(
        "beagle serve: push: {} {} → device {}: {}",
        p.record.tool, p.record.kind, p.device, outcome.message
    );
    outcome
}

/// One round: new events, matched against the phones, handed to `sender`.
/// Returns how many notifications went to the sender.
pub fn tick(follower: &mut Follower, store: &DeviceStore, sender: &SharedSender) -> usize {
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
            deliver(sender, &pending);
            sent += 1;
        }
    }
    sent
}

/// A test notification for phone `id`, or `None` when it has no token.
#[must_use]
pub fn test_pending(id: &str, device: &Device) -> Option<Pending> {
    Some(Pending {
        device: id.to_owned(),
        apns_token: device.apns_token.clone()?,
        environment: device.environment,
        topic: device.topic.clone(),
        record: Record {
            v: beagle_scout::events::VERSION,
            id: random_uuid(),
            time: OffsetDateTime::now_utc()
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap_or_default(),
            tool: "beagle".into(),
            kind: "test".into(),
            project: "test".into(),
            project_title: String::new(),
            task: None,
            task_title: None,
            title: "Beagle · test push".into(),
            subtitle: format!("To {}", device.name),
            body: "Push notifications from your Mac work.".into(),
            link: "beagle://".into(),
        },
    })
}

/// Sends a test push to phone `id` through `sender`.
#[must_use]
pub fn send_test(sender: &SharedSender, id: &str, device: &Device) -> Outcome {
    match test_pending(id, device) {
        Some(p) => deliver(sender, &p),
        None => Outcome::failed("the phone has no push token yet: open the app on it once".into()),
    }
}

/// A random UUID v4 (lowercase), the shape `apns-id` takes.
fn random_uuid() -> String {
    use ring::rand::SecureRandom as _;
    let mut b = [0u8; 16];
    // A failing system RNG leaves zeros: still a valid, if unlucky, id.
    let _ = ring::rand::SystemRandom::new().fill(&mut b);
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let hex = b.iter().fold(String::with_capacity(32), |mut hex, x| {
        use std::fmt::Write as _;
        let _ = write!(hex, "{x:02x}");
        hex
    });
    format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    )
}

/// Starts the background loop: follows every tool's spool and hands
/// matches to `sender`.
///
/// # Errors
/// [`Error::NoConfigDir`] without a home directory, or the thread's error.
pub fn spawn(store: Arc<DeviceStore>, sender: SharedSender) -> Result<()> {
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
            tick(&mut follower, &store, &sender);
            std::thread::sleep(POLL_EVERY);
        })
        .map(|_| ())
        .map_err(|e| Error::Push(format!("could not start the push thread: {e}")))
}

/// Push's state for `beagle serve status` and `beagle doctor`: `Ok` with a
/// summary when configured and the key loads, `Err` with what to do.
///
/// # Errors
/// Not set up, or set up but broken, as a line for a person.
pub fn status() -> std::result::Result<String, String> {
    let path = config::path().map_err(|e| e.to_string())?;
    let config = config::load(&path).map_err(|e| e.to_string())?;
    let push = config.push.ok_or_else(|| {
        "not set up — run `beagle serve push setup` (docs/serve-push.md)".to_owned()
    })?;
    cmd::signer(&push)?;
    Ok(format!(
        "configured (key {}, team {})",
        push.key_id, push.team_id
    ))
}

#[cfg(test)]
#[path = "../tests/push.rs"]
mod tests;
