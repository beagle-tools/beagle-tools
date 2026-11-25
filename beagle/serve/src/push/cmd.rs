//! `beagle serve push setup` and `beagle serve push test`, and loading the
//! key they and the server share.

use std::path::Path;
use std::sync::{Arc, Mutex};

use super::devices::DeviceStore;
use super::jwt::Signer;
use super::{devices_path, send_test, sender_for, SharedSender};
use crate::config::{self, PushConfig};
use crate::error::{Error, Result};
use crate::service;

/// The signer for `push`: checks the ids, refuses a key others can read,
/// and loads it.
///
/// # Errors
/// What is wrong, for a person.
pub fn signer(push: &PushConfig) -> std::result::Result<Signer, String> {
    push.validate_ids()?;
    let path = push.key_path();
    if config::readable_by_others(&path) {
        return Err(format!(
            "{} is readable by others; it is your APNs key: chmod 600 it",
            path.display()
        ));
    }
    let pem = std::fs::read_to_string(&path)
        .map_err(|e| format!("can't read the APNs key {}: {e}", path.display()))?;
    Signer::new(&pem, &push.team_id, &push.key_id).map_err(|e| format!("{}: {e}", path.display()))
}

/// `beagle serve push setup --key <file> --key-id <id> --team-id <id>`:
/// checks the key, copies it next to `serve.toml` (0600) and writes `[push]`.
///
/// # Errors
/// A bad key or id ([`Error::Usage`]), or the file operations' errors.
pub fn setup(key: &Path, key_id: &str, team_id: &str) -> Result<()> {
    let pem = std::fs::read_to_string(key).map_err(Error::io(key))?;
    let candidate = PushConfig {
        team_id: team_id.to_owned(),
        key_id: key_id.to_owned(),
        key: key.to_owned(),
    };
    candidate.validate_ids().map_err(Error::Usage)?;
    Signer::new(&pem, team_id, key_id)
        .map_err(|e| Error::Usage(format!("{}: {e}", key.display())))?;

    let dir = config::cert_dir()?;
    std::fs::create_dir_all(&dir).map_err(Error::io(&dir))?;
    let dest = dir.join(format!("AuthKey_{key_id}.p8"));
    beagle_utils::fs::write_atomic_with_mode(&dest, pem.as_bytes(), 0o600)
        .map_err(Error::io(&dest))?;

    let path = config::path()?;
    let mut config = config::load(&path)?;
    config.push = Some(PushConfig {
        key: dest.clone(),
        ..candidate
    });
    config::save(&path, &config)?;
    println!("APNs key saved to {} (mode 0600).", dest.display());
    println!("[push] written to {}.", path.display());
    println!("You can delete {} now.", key.display());
    match service::restart_if_loaded() {
        Ok(true) => println!("Restarted the background service: push is on."),
        _ => println!("Restart `beagle serve` if it is running, so push turns on."),
    }
    println!("Then: `beagle serve push test`.");
    Ok(())
}

/// `beagle serve push test [--device <id>]`: one test push to every
/// registered phone (or one), printing APNs' answer for each. Fails when
/// none was delivered.
///
/// # Errors
/// Push not set up, no phones, or no delivery ([`Error::Push`]).
pub fn test(device: Option<&str>) -> Result<()> {
    let path = config::path()?;
    let config = config::load(&path)?;
    let store = Arc::new(DeviceStore::new(devices_path()?));
    let sender = sender_for(&config, Arc::clone(&store)).map_err(Error::Push)?;
    if !sender.configured() {
        return Err(Error::Push(
            "push isn't set up: run `beagle serve push setup` first".into(),
        ));
    }
    let shared: SharedSender = Arc::new(Mutex::new(sender));
    let devices = store.load()?;
    let chosen: Vec<_> = devices
        .devices
        .iter()
        .filter(|(id, _)| device.is_none_or(|d| d.eq_ignore_ascii_case(id)))
        .collect();
    if chosen.is_empty() {
        return Err(Error::Push(match device {
            Some(d) => format!("no phone {d} is registered"),
            None => "no phones registered: open the Beagle app and turn on notifications".into(),
        }));
    }
    let mut delivered = 0;
    for (id, d) in chosen {
        let outcome = send_test(&shared, id, d);
        println!("{id} ({}): {}", d.name, outcome.message);
        delivered += usize::from(outcome.delivered);
    }
    if delivered == 0 {
        return Err(Error::Push("no test push was delivered".into()));
    }
    Ok(())
}
