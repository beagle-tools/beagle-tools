//! Listing workspaces: active, archived, and both, with corrupt or
//! unreadable workspaces kept as [`BrokenWorkspace`] rows.

use std::fs;
use std::path::Path;

use crate::error::{Error, Result};
use crate::model::{RcaId, RcaMeta, RcaSummary};

use super::fsio::read_bounded;
use super::{BrokenWorkspace, Listing, LoadWarning, Store, ARCHIVE_DIR, MANIFEST_FILE};

impl Store {
    /// Lists every workspace, sorted for the sidebar (open incidents first,
    /// then severity, then newest). Unreadable or corrupt workspaces are
    /// returned as [`Listing::broken`] — never silently dropped: a
    /// workspace that exists on disk must stay visible, with the reason it
    /// could not load.
    ///
    /// This reads only the small manifests — section content stays on disk
    /// until a tab asks for it.
    ///
    /// # Errors
    /// Returns [`Error::Io`] only if the `rcas/` directory itself cannot be
    /// read.
    pub fn list(&self) -> Result<Listing> {
        Self::list_dir(&self.rcas_root, false)
    }

    /// Lists the workspaces under `rcas/archive/`, sorted like
    /// [`Self::list`]. A missing archive directory is an empty listing, not
    /// an error.
    ///
    /// # Errors
    /// Returns [`Error::Io`] only if the archive directory exists but
    /// cannot be read.
    pub fn list_archived(&self) -> Result<Listing> {
        let archive_root = self.rcas_root.join(ARCHIVE_DIR);
        if !archive_root.is_dir() {
            return Ok(Listing::default());
        }
        Self::list_dir(&archive_root, true)
    }

    /// Active and archived workspaces in one listing, re-sorted so archived
    /// incidents sink below everything active. This is what the TUI loads;
    /// it hides the archived entries until asked.
    ///
    /// # Errors
    /// As [`Self::list`]; an unreadable archive directory degrades to a
    /// warning rather than failing the whole listing.
    pub fn list_all(&self) -> Result<Listing> {
        let mut listing = self.list()?;
        match self.list_archived() {
            Ok(mut archived) => {
                listing.summaries.append(&mut archived.summaries);
                listing.broken.append(&mut archived.broken);
                listing.warnings.append(&mut archived.warnings);
            }
            Err(e) => listing
                .warnings
                .push(LoadWarning(format!("archive unreadable: {e}"))),
        }
        listing.summaries.sort_by_key(RcaSummary::sort_key);
        listing.broken.sort_by(|a, b| a.dir_name.cmp(&b.dir_name));
        Ok(listing)
    }

    fn list_dir(root: &Path, archived: bool) -> Result<Listing> {
        let entries = fs::read_dir(root).map_err(|e| Error::io(root, e))?;

        let mut listing = Listing::default();
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    listing
                        .warnings
                        .push(LoadWarning(format!("unreadable directory entry: {e}")));
                    continue;
                }
            };
            let path = entry.path();
            if !path.is_dir() {
                continue; // stray files next to workspaces are fine to ignore
            }
            let dir_name = entry.file_name().to_string_lossy().into_owned();
            if !archived && dir_name == ARCHIVE_DIR {
                continue; // archived workspaces list via `list_archived`
            }
            match Self::load_summary(&dir_name, &path) {
                Ok(mut summary) => {
                    summary.archived = archived;
                    listing.summaries.push(summary);
                }
                Err(e) => listing.broken.push(BrokenWorkspace {
                    dir_name,
                    reason: Self::broken_reason(&e),
                }),
            }
        }
        listing.summaries.sort_by_key(RcaSummary::sort_key);
        listing.broken.sort_by(|a, b| a.dir_name.cmp(&b.dir_name));
        Ok(listing)
    }

    /// A compact, sidebar-friendly reason for a workspace that failed to
    /// load. The broken row already shows the directory name, so the
    /// manifest's absolute path — which is all a narrow sidebar would have
    /// room for — is dropped in favor of what actually went wrong.
    fn broken_reason(e: &Error) -> String {
        match e {
            Error::Io { source, .. } if source.kind() == std::io::ErrorKind::NotFound => {
                format!("no {MANIFEST_FILE} — not a beagle workspace")
            }
            Error::ParseManifest { source, .. } => {
                format!("{MANIFEST_FILE}: {}", source.message())
            }
            _ => e.to_string(),
        }
    }

    pub(super) fn load_summary(dir_name: &str, dir: &Path) -> Result<RcaSummary> {
        let id = RcaId::new(dir_name)?;
        let manifest_path = dir.join(MANIFEST_FILE);
        let raw = read_bounded(&manifest_path)?;
        let meta: RcaMeta = toml::from_str(&raw).map_err(|source| Error::ParseManifest {
            path: manifest_path,
            source: Box::new(source),
        })?;
        Ok(RcaSummary {
            id,
            meta,
            archived: false,
        })
    }
}
