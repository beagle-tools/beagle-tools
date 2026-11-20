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
}

impl Stores {
    /// Resolves every store as its tool does.
    #[must_use]
    pub fn resolve() -> Self {
        Self {
            scout: beagle_scout::default_root().map_err(|e| e.to_string()),
        }
    }

    /// One line per store, for the startup log.
    #[must_use]
    pub fn describe(&self) -> Vec<String> {
        [("scout", &self.scout)]
            .into_iter()
            .map(|(tool, source)| match source {
                Ok(path) => format!("{tool}: {}", path.display()),
                Err(e) => format!("{tool}: unavailable ({e})"),
            })
            .collect()
    }
}
