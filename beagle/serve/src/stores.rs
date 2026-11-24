//! Where each tool's store is, resolved once at startup the same way the
//! tool itself resolves it when run with no flags.
//!
//! A tool whose store cannot be resolved (a broken config file, no home
//! directory) does not stop the server: its endpoints answer `500` with
//! the reason, and every other tool keeps working.

use std::path::PathBuf;

/// A store's root, or why it could not be found.
pub type Source = Result<PathBuf, String>;

/// Every tool's store.
#[derive(Debug, Clone)]
pub struct Stores {
    /// `beagle scout`'s store root.
    pub scout: Source,
    /// `beagle rca`'s store root (the directory holding `rcas/`).
    pub rca: Source,
    /// `beagle recap`'s store root (the directory holding `recaps/`).
    pub recap: Source,
    /// `beagle standup`'s store root (the directory holding `standups/`).
    pub standup: Source,
    /// The Tandem review TUI's store: the SQLite file itself.
    pub tandem: Source,
}

impl Stores {
    /// Resolves every store as its tool does.
    #[must_use]
    pub fn resolve() -> Self {
        Self {
            scout: beagle_scout::default_root().map_err(|e| e.to_string()),
            rca: beagle_rca::effective_root(None).map_err(|e| e.to_string()),
            recap: ::recap::config::root_for(None).map_err(|e| e.to_string()),
            standup: ::standup::config::root_for(None).map_err(|e| e.to_string()),
            tandem: tandem_tui::read::store_path().map_err(|e| e.to_string()),
        }
    }

    /// One line per store, for the startup log.
    #[must_use]
    pub fn describe(&self) -> Vec<String> {
        [
            ("scout", &self.scout),
            ("rca", &self.rca),
            ("recap", &self.recap),
            ("standup", &self.standup),
            ("tandem", &self.tandem),
        ]
        .into_iter()
        .map(|(tool, source)| match source {
            Ok(path) => format!("{tool}: {}", path.display()),
            Err(e) => format!("{tool}: unavailable ({e})"),
        })
        .collect()
    }
}
