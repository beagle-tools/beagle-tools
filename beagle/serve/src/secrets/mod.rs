//! `beagle serve secrets sync|status|clear`: push's APNs key, copied from
//! a secret manager into the macOS Keychain (`docs/serve-push.md`,
//! "Keeping the key in a secret manager").
//!
//! ```text
//! <your command> ──stdout (JSON / dotenv)──▶ parse ──▶ BEAGLE_APNS_* only ──▶ Keychain
//!                                                                              │
//! beagle serve ◀── env > Keychain > serve.toml [push] ◀─────────────────────────┘
//! ```
//!
//! Only the three `BEAGLE_APNS_*` names are kept; everything else the
//! command prints is dropped. Values are never printed or logged.

pub mod keychain;
pub mod parse;

use std::process::Command;
use std::time::Duration;

use beagle_utils::process::output_with_timeout;

use crate::config::{self, SecretsConfig};
use crate::error::{Error, Result};
use crate::push::source::{self, ENV_VARS};
pub use keychain::{Keychain, NEEDS_APPROVAL};

/// Bound on the secret manager's command.
pub const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);

/// Where synced secrets live: the Keychain, or a map in tests.
pub trait SecretStore {
    /// The value of `name`, `None` when absent.
    ///
    /// # Errors
    /// The store's error, for a person.
    fn get(&self, name: &str) -> std::result::Result<Option<String>, String>;

    /// Stores `value` as `name`, replacing any old one.
    ///
    /// # Errors
    /// The store's error, for a person.
    fn set(&self, name: &str, value: &str) -> std::result::Result<(), String>;

    /// Removes `name`; `false` when it wasn't there.
    ///
    /// # Errors
    /// The store's error, for a person.
    fn delete(&self, name: &str) -> std::result::Result<bool, String>;
}

/// Runs `command` with `sh -c` (bounded by `timeout`) and returns its
/// stdout.
///
/// # Errors
/// It could not start, timed out, or exited non-zero (with its stderr,
/// which a secret manager uses for errors, not values).
pub fn run_command(command: &str, timeout: Duration) -> std::result::Result<String, String> {
    let out = output_with_timeout(Command::new("sh").arg("-c").arg(command), timeout)
        .map_err(|e| format!("could not run the secrets command: {e}"))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let stderr: String = stderr.trim().chars().take(300).collect();
        return Err(format!(
            "the secrets command failed ({}){}",
            out.status,
            if stderr.is_empty() {
                String::new()
            } else {
                format!(": {stderr}")
            }
        ));
    }
    String::from_utf8(out.stdout).map_err(|_| "the secrets command printed non-UTF-8".to_owned())
}

/// Stores the `BEAGLE_APNS_*` values in `output` into `store`, after
/// checking they make a working key. Returns the names stored.
///
/// # Errors
/// Output that doesn't parse, lacks a name, or holds a bad key or id —
/// nothing is stored then. Never contains a value.
pub fn sync_output(
    store: &dyn SecretStore,
    output: &str,
) -> std::result::Result<Vec<&'static str>, String> {
    let all = parse::parse(output)?;
    let values: Vec<Option<String>> = ENV_VARS
        .iter()
        .map(|name| all.get(*name).filter(|v| !v.trim().is_empty()).cloned())
        .collect();
    if values.iter().all(Option::is_none) {
        return Err(format!(
            "the secrets command printed none of {}",
            ENV_VARS.join(", ")
        ));
    }
    source::from_values(&values, source::Source::Keychain)?;
    for (name, value) in ENV_VARS.iter().zip(&values) {
        if let Some(value) = value {
            store.set(name, value)?;
        }
    }
    Ok(ENV_VARS.to_vec())
}

/// Which of the names `store` holds (never their values).
#[must_use]
pub fn present(store: &dyn SecretStore) -> Vec<(&'static str, std::result::Result<bool, String>)> {
    ENV_VARS
        .iter()
        .map(|name| (*name, store.get(name).map(|v| v.is_some())))
        .collect()
}

/// Removes every name from `store`; returns the ones that were there.
///
/// # Errors
/// The store's error.
pub fn clear(store: &dyn SecretStore) -> std::result::Result<Vec<&'static str>, String> {
    let mut removed = Vec::new();
    for name in ENV_VARS {
        if store.delete(name)? {
            removed.push(name);
        }
    }
    Ok(removed)
}

/// `beagle serve secrets sync [--from <command>]`: runs the command (or the
/// one saved in `serve.toml`), stores the values in the Keychain, and saves
/// the command for next time.
///
/// # Errors
/// No command, or the sync's error ([`Error::Secrets`]).
pub fn sync_command(from: Option<&str>) -> Result<()> {
    let path = config::path()?;
    let mut config = config::load(&path)?;
    let command = from
        .map(str::to_owned)
        .or_else(|| config.secrets.as_ref().map(|s| s.command.clone()))
        .ok_or_else(|| {
            Error::Usage(
                "no secrets command saved yet: `beagle serve secrets sync --from \"<command>\"`"
                    .into(),
            )
        })?;
    let output = run_command(&command, COMMAND_TIMEOUT).map_err(Error::Secrets)?;
    let stored = sync_output(&Keychain::default(), &output).map_err(Error::Secrets)?;
    println!(
        "Stored {} in the Keychain (service {}).",
        stored.join(", "),
        keychain::SERVICE
    );
    if from.is_some() && config.secrets.as_ref().map(|s| s.command.as_str()) != from {
        config.secrets = Some(SecretsConfig { command });
        config::save(&path, &config)?;
        println!(
            "Saved the command as [secrets] in {}: `beagle serve secrets sync` reruns it, and `beagle serve` at startup.",
            path.display()
        );
    }
    match crate::service::restart_if_loaded() {
        Ok(true) => println!("Restarted the background service: it uses the new key."),
        _ => println!("Restart `beagle serve` if it is running, so it uses the new key."),
    }
    Ok(())
}

/// `beagle serve secrets status`: which names the Keychain holds, the
/// saved command, and where push config comes from.
///
/// # Errors
/// The config's error.
pub fn status_command() -> Result<()> {
    let config = config::load(&config::path()?)?;
    for (name, present) in present(&Keychain::non_interactive()) {
        let state = match present {
            Ok(true) => "in the Keychain".to_owned(),
            Ok(false) => "not stored".to_owned(),
            Err(e) => format!("unreadable ({e})"),
        };
        let env = if source::process_env(name).is_some_and(|v| !v.trim().is_empty()) {
            " (also set in this shell's environment, which wins)"
        } else {
            ""
        };
        println!("{name}: {state}{env}");
    }
    println!(
        "command: {}",
        config
            .secrets
            .as_ref()
            .map_or("none saved", |s| s.command.as_str())
    );
    println!("push: {}", crate::push::status().unwrap_or_else(|why| why));
    Ok(())
}

/// `beagle serve secrets clear`: removes the names from the Keychain.
///
/// # Errors
/// The Keychain's error ([`Error::Secrets`]).
pub fn clear_command() -> Result<()> {
    let removed = clear(&Keychain::default()).map_err(Error::Secrets)?;
    if removed.is_empty() {
        println!("Nothing to remove: no BEAGLE_APNS_* items in the Keychain.");
    } else {
        println!("Removed {} from the Keychain.", removed.join(", "));
    }
    println!("The [secrets] command in serve.toml is kept; `beagle serve` reruns it at startup.");
    Ok(())
}

/// The outcome of a sync: the names stored, or why nothing was.
pub type SyncResult = std::result::Result<Vec<&'static str>, String>;

/// At `beagle serve` startup, on its own thread so the server answers at
/// once with what `store` already has: reruns the saved `command` (bounded
/// by [`COMMAND_TIMEOUT`]; a secret manager can be slow, or wait on a
/// locked screen), stores its values, logs the result and hands it to
/// `done`. A failure keeps what the store already has.
///
/// # Errors
/// The thread could not be started.
pub fn spawn_startup_sync<S>(
    command: String,
    store: S,
    done: impl FnOnce(&SyncResult) + Send + 'static,
) -> std::io::Result<std::thread::JoinHandle<()>>
where
    S: SecretStore + Send + 'static,
{
    std::thread::Builder::new()
        .name("serve-secrets-sync".into())
        .spawn(move || {
            let result =
                run_command(&command, COMMAND_TIMEOUT).and_then(|out| sync_output(&store, &out));
            match &result {
                Ok(stored) => eprintln!("beagle serve:   secrets: synced {}", stored.join(", ")),
                Err(why) => eprintln!(
                    "beagle serve:   secrets: sync failed, using what the Keychain has: {why}"
                ),
            }
            done(&result);
        })
}

#[cfg(test)]
#[path = "../tests/secrets.rs"]
mod tests;

/// An in-memory [`SecretStore`] and a throwaway key, so tests never touch
/// the real Keychain.
#[cfg(test)]
#[allow(clippy::expect_used)]
pub(crate) mod tests_support {
    use std::cell::RefCell;
    use std::collections::BTreeMap;

    use base64::engine::general_purpose::STANDARD;
    use base64::Engine as _;
    use ring::rand::SystemRandom;
    use ring::signature::{EcdsaKeyPair, ECDSA_P256_SHA256_FIXED_SIGNING};

    use super::SecretStore;
    use crate::push::source::{ENV_KEY, ENV_KEY_ID, ENV_TEAM_ID};

    /// A [`SecretStore`] in a map.
    #[derive(Debug, Default)]
    pub(crate) struct MemoryStore(pub RefCell<BTreeMap<String, String>>);

    impl MemoryStore {
        /// All three names set.
        pub(crate) fn full(pem: &str, key_id: &str, team_id: &str) -> Self {
            let store = Self::default();
            for (k, v) in [(ENV_KEY, pem), (ENV_KEY_ID, key_id), (ENV_TEAM_ID, team_id)] {
                store.0.borrow_mut().insert(k.to_owned(), v.to_owned());
            }
            store
        }
    }

    impl SecretStore for MemoryStore {
        fn get(&self, name: &str) -> Result<Option<String>, String> {
            Ok(self.0.borrow().get(name).cloned())
        }

        fn set(&self, name: &str, value: &str) -> Result<(), String> {
            self.0
                .borrow_mut()
                .insert(name.to_owned(), value.to_owned());
            Ok(())
        }

        fn delete(&self, name: &str) -> Result<bool, String> {
            Ok(self.0.borrow_mut().remove(name).is_some())
        }
    }

    /// A fresh P-256 key as a `.p8` file's text.
    pub(crate) fn p8() -> String {
        let doc =
            EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &SystemRandom::new())
                .expect("generate a throwaway key");
        let b64 = STANDARD.encode(doc.as_ref());
        let lines: Vec<&str> = b64
            .as_bytes()
            .chunks(64)
            .map(|c| std::str::from_utf8(c).unwrap_or_default())
            .collect();
        format!(
            "-----BEGIN PRIVATE KEY-----\n{}\n-----END PRIVATE KEY-----\n",
            lines.join("\n")
        )
    }
}
