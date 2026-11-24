//! Where agent env values live: NOT the cache DB.
//!
//! An agent spec's `env` often holds API keys. The spec itself is stored as
//! JSON in `agent_specs`, but its env goes to `agent-env.json` beside the
//! database, written atomically with mode 0600, and is merged back when a
//! spec is read. Callers see whole specs and never know the difference.
//!
//! Specs saved by older versions still carry env in their row; the first
//! read moves it out (see `ReviewStore::list_agent_specs`).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

use tandem_core::{Result, TandemError};

/// Agent name → its env vars.
pub(crate) type EnvMap = BTreeMap<String, BTreeMap<String, String>>;

/// The env file next to a database, or memory for in-memory caches.
#[derive(Debug)]
pub(crate) enum EnvStore {
    File(PathBuf),
    Memory(Mutex<EnvMap>),
}

impl EnvStore {
    /// `agent-env.json` in the same directory as `db`.
    pub(crate) fn beside(db: &std::path::Path) -> Self {
        Self::File(db.with_file_name("agent-env.json"))
    }

    pub(crate) fn memory() -> Self {
        Self::Memory(Mutex::new(EnvMap::new()))
    }

    fn load(&self) -> Result<EnvMap> {
        match self {
            Self::File(path) => match beagle_utils::fs::read_optional(path)? {
                Some(text) => Ok(serde_json::from_str(&text)?),
                None => Ok(EnvMap::new()),
            },
            Self::Memory(map) => Ok(map.lock().map_err(poisoned)?.clone()),
        }
    }

    fn save(&self, all: &EnvMap) -> Result<()> {
        match self {
            Self::File(path) => {
                let text = serde_json::to_string_pretty(all)?;
                beagle_utils::fs::write_atomic_with_mode(path, text.as_bytes(), 0o600)?;
                Ok(())
            }
            Self::Memory(map) => {
                *map.lock().map_err(poisoned)? = all.clone();
                Ok(())
            }
        }
    }

    /// The env for `agent` (empty when none is stored).
    pub(crate) fn get(&self, agent: &str) -> Result<BTreeMap<String, String>> {
        Ok(self.load()?.remove(agent).unwrap_or_default())
    }

    /// Stores (or, when empty, removes) the env for `agent`.
    pub(crate) fn set(&self, agent: &str, env: &BTreeMap<String, String>) -> Result<()> {
        let mut all = self.load()?;
        let changed = if env.is_empty() {
            all.remove(agent).is_some()
        } else {
            all.insert(agent.to_owned(), env.clone()).as_ref() != Some(env)
        };
        if changed {
            self.save(&all)?;
        }
        Ok(())
    }
}

fn poisoned<T>(_: T) -> TandemError {
    TandemError::Cache("agent env store lock poisoned".into())
}
