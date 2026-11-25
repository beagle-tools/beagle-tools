//! Where push's APNs key comes from, first match wins:
//!
//! 1. the environment (`BEAGLE_APNS_*`, e.g. injected by a secret manager),
//! 2. the macOS Keychain (`beagle serve secrets sync`, [`crate::secrets`]),
//! 3. `[push]` in `serve.toml` (`beagle serve push setup`).
//!
//! A key from the environment or the Keychain stays in memory: it is never
//! written to disk and never logged. Only the source and the two ids are
//! ever reported.

use std::path::PathBuf;

use super::jwt::Signer;
use crate::config::{Config, PushConfig};
use crate::secrets::SecretStore;

/// The `.p8` key's text (PEM). Escaped newlines (`\n`) are accepted.
pub const ENV_KEY: &str = "BEAGLE_APNS_KEY";
/// The key's Key ID.
pub const ENV_KEY_ID: &str = "BEAGLE_APNS_KEY_ID";
/// The developer account's Team ID.
pub const ENV_TEAM_ID: &str = "BEAGLE_APNS_TEAM_ID";
/// All three, in that order. Setting any of them in a source means "use
/// this source".
pub const ENV_VARS: [&str; 3] = [ENV_KEY, ENV_KEY_ID, ENV_TEAM_ID];

/// Where the push config came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// `BEAGLE_APNS_*` environment variables.
    Env,
    /// `BEAGLE_APNS_*` Keychain items.
    Keychain,
    /// `[push]` in `serve.toml`.
    File,
}

impl Source {
    /// The source, for a person.
    #[must_use]
    pub const fn describe(self) -> &'static str {
        match self {
            Self::Env => "the environment (BEAGLE_APNS_*)",
            Self::Keychain => "the Keychain (beagle serve secrets sync)",
            Self::File => "serve.toml [push]",
        }
    }
}

/// A loaded key and where it came from.
#[derive(Debug)]
pub struct Loaded {
    /// The key, ready to sign.
    pub signer: Signer,
    /// Where it came from.
    pub source: Source,
    /// Its Key ID.
    pub key_id: String,
    /// The Team ID.
    pub team_id: String,
}

impl Loaded {
    /// One line: the source and the ids, never the key.
    #[must_use]
    pub fn summary(&self) -> String {
        format!(
            "configured from {} (key {}, team {})",
            self.source.describe(),
            self.key_id,
            self.team_id
        )
    }
}

/// The process environment, for [`resolve`].
#[must_use]
pub fn process_env(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

/// The push config from `env`, then `store`, then `config`: `Ok(None)`
/// when none sets one. The first source holding any `BEAGLE_APNS_*` name
/// is used whole, so a broken source turns push off rather than silently
/// falling through to the next.
///
/// # Errors
/// What is wrong, for a person; never contains key material.
pub fn resolve(
    config: &Config,
    env: &dyn Fn(&str) -> Option<String>,
    store: &dyn SecretStore,
) -> Result<Option<Loaded>, String> {
    let from_env: Vec<Option<String>> = ENV_VARS.iter().map(|n| nonblank(env(n))).collect();
    if from_env.iter().any(Option::is_some) {
        return from_values(&from_env, Source::Env).map(Some);
    }
    let mut from_store = Vec::with_capacity(ENV_VARS.len());
    for name in ENV_VARS {
        from_store.push(nonblank(
            store.get(name).map_err(|e| format!("{name}: {e}"))?,
        ));
    }
    if from_store.iter().any(Option::is_some) {
        return from_values(&from_store, Source::Keychain).map(Some);
    }
    let Some(push) = &config.push else {
        return Ok(None);
    };
    Ok(Some(Loaded {
        signer: super::cmd::signer(push)?,
        source: Source::File,
        key_id: push.key_id.clone(),
        team_id: push.team_id.clone(),
    }))
}

fn nonblank(value: Option<String>) -> Option<String> {
    value.filter(|v| !v.trim().is_empty())
}

/// The key from `values` (in [`ENV_VARS`] order), all three required.
///
/// # Errors
/// A missing name, a bad id, or a key that doesn't load; never a value of
/// the key.
pub fn from_values(values: &[Option<String>], source: Source) -> Result<Loaded, String> {
    let missing: Vec<&str> = ENV_VARS
        .iter()
        .zip(values)
        .filter(|(_, v)| v.is_none())
        .map(|(name, _)| *name)
        .collect();
    if !missing.is_empty() || values.len() != ENV_VARS.len() {
        return Err(format!(
            "{} not set in {}; push needs all of {}",
            missing.join(", "),
            source.describe(),
            ENV_VARS.join(", ")
        ));
    }
    let value = |i: usize| values.get(i).cloned().flatten().unwrap_or_default();
    let ids = PushConfig {
        key_id: value(1).trim().to_owned(),
        team_id: value(2).trim().to_owned(),
        key: PathBuf::new(),
    };
    ids.validate_ids()
        .map_err(|e| format!("{e} (from {ENV_KEY_ID} / {ENV_TEAM_ID})"))?;
    let signer = Signer::new(&unescape_newlines(&value(0)), &ids.team_id, &ids.key_id)
        .map_err(|e| format!("{ENV_KEY}: {e}"))?;
    Ok(Loaded {
        signer,
        source,
        key_id: ids.key_id,
        team_id: ids.team_id,
    })
}

/// A PEM stored on one line with literal `\n`s, as some secret managers
/// and `.env` files keep it, back to real lines.
fn unescape_newlines(pem: &str) -> String {
    if pem.contains('\n') {
        pem.to_owned()
    } else {
        pem.replace("\\n", "\n")
    }
}

#[cfg(test)]
#[path = "../tests/push_source.rs"]
mod tests;
