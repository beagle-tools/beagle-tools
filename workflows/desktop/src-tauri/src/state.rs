//! Application state: one handle onto the same SQLite store the daemon
//! writes.
//!
//! `agent_core::Store` is explicitly documented as "one per thread — it is
//! not `Sync`", so it lives behind a `Mutex` and every command holds the
//! lock only for the length of its query. Nothing caches rows: the daemon
//! is the single writer and the store is already WAL, so a read is cheap
//! and always current. A cache here would be a second source of truth for
//! no gain.

use std::sync::Mutex;

use agent_core::state::Store;

use crate::error::AppError;

#[derive(Debug)]
pub struct AppState {
    pub store: Mutex<Store>,
}

impl AppState {
    pub fn init() -> Result<Self, AppError> {
        let store = Store::open_default()
            .map_err(|e| AppError::new(format!("could not open the agentd store: {e}")))?;
        Ok(Self {
            store: Mutex::new(store),
        })
    }
}
