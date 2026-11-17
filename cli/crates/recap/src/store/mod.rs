//! The recap store: the directory tree under the store root.
//!
//! ```text
//! <root>/
//!  recaps/
//!   widgets-42/               one per PR: <repo>-<number>
//!     recap.toml              PR metadata (+ ticket)
//!     summary.md              the write-up
//!     diff.patch              the PR's unified diff
//!     diagrams/NN-*.txt       ASCII diagrams
//! ```
//!
//! Every read is size-capped, and anything malformed degrades to a warning
//! rather than an error — recaps are written by agents and must never take
//! the TUI down.

mod export;
mod files;
mod state;

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde::Serialize;

pub use export::export_markdown;
use files::{create_dir, read_diagrams, read_text, read_toml, write_atomic};
pub use state::{now_unix, AUTO_ARCHIVE_AFTER_SECS};

use crate::error::{Error, Result};
use crate::model::{PrState, RecapId, RecapManifest};

/// The directory under the store root that holds the recaps.
pub const RECAPS_DIR: &str = "recaps";

/// The archive shelf, under `recaps/`. The leading underscore can't appear
/// in a recap id, so the active list never mistakes it for a recap.
pub const ARCHIVE_DIR: &str = "_archive";

/// Which set of recaps: the active list, or the archive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shelf {
    /// `recaps/<id>/`.
    Active,
    /// `recaps/_archive/<id>/`.
    Archived,
}

/// Largest text file the store will load (2 MiB).
pub const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;
/// Largest diff the store will load (16 MiB).
pub const MAX_DIFF_BYTES: u64 = 16 * 1024 * 1024;

pub(crate) const RECAP_TOML: &str = "recap.toml";
const SUMMARY_MD: &str = "summary.md";
const DIFF_PATCH: &str = "diff.patch";
const DIAGRAMS: &str = "diagrams";

const SUMMARY_TEMPLATE: &str = "\
## TL;DR

## Where this sits in the stack

## The problem

## Why this PR is needed

## The fix

## How to review it

## Where it stands

## What we need to do
";

/// The nearest directory at or above `start` that already contains a
/// `recaps/` directory, or `start` itself when none does (git-style).
#[must_use]
pub fn discover_root(start: &Path) -> PathBuf {
    crate::config::DIRS.discover_root(start)
}

/// One row of `beagle recap list`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecapSummary {
    /// The recap id.
    pub id: String,
    /// `owner/repo`.
    pub repo: String,
    /// PR number.
    pub number: u64,
    /// PR title.
    pub title: String,
    /// PR state.
    pub state: PrState,
    /// Ticket id, if any.
    pub ticket: Option<String>,
    /// When the PR was opened.
    pub created: Option<String>,
    /// PR URL.
    pub url: String,
}

/// A diagram file: its name (without extension) and raw body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagram {
    /// File stem, e.g. `01-request-path`.
    pub name: String,
    /// Raw contents, possibly with ANSI colour codes.
    pub body: String,
}

/// A fully loaded recap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recap {
    /// The id (directory name).
    pub id: RecapId,
    /// `recap.toml`.
    pub manifest: RecapManifest,
    /// `summary.md`.
    pub summary: String,
    /// `diff.patch`.
    pub diff: String,
    /// `diagrams/*.txt`, in file-name order.
    pub diagrams: Vec<Diagram>,
    /// Problems skipped over while loading.
    pub warnings: Vec<String>,
}

impl Recap {
    /// The PR state bucket.
    #[must_use]
    pub fn state(&self) -> PrState {
        self.manifest.state
    }
}

/// Handle on a store root.
#[derive(Debug, Clone)]
pub struct Store {
    root: PathBuf,
}

impl Store {
    /// A store rooted at `root` (which need not exist yet); recaps live in
    /// `root/recaps/`.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The store root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// `<root>/recaps/`.
    #[must_use]
    pub fn recaps_dir(&self) -> PathBuf {
        self.root.join(RECAPS_DIR)
    }

    /// The directory of recap `id`.
    #[must_use]
    pub fn recap_dir(&self, id: &RecapId) -> PathBuf {
        self.recaps_dir().join(id.as_str())
    }

    /// The directory a shelf lives in.
    #[must_use]
    pub fn shelf_dir(&self, which: Shelf) -> PathBuf {
        match which {
            Shelf::Active => self.recaps_dir(),
            Shelf::Archived => self.recaps_dir().join(ARCHIVE_DIR),
        }
    }

    /// Moves recap `id` to the archive. The files are kept; it just leaves
    /// the active list.
    ///
    /// # Errors
    /// [`Error::NoSuchRecap`], [`Error::AlreadyExists`] (already archived),
    /// or [`Error::Io`].
    pub fn archive(&self, id: &RecapId) -> Result<PathBuf> {
        self.move_between(id, Shelf::Active, Shelf::Archived)
    }

    /// Moves recap `id` back from the archive.
    ///
    /// # Errors
    /// [`Error::NoSuchRecap`], [`Error::AlreadyExists`] (an active recap with
    /// that id exists), or [`Error::Io`].
    pub fn unarchive(&self, id: &RecapId) -> Result<PathBuf> {
        self.move_between(id, Shelf::Archived, Shelf::Active)
    }

    fn move_between(&self, id: &RecapId, from: Shelf, to: Shelf) -> Result<PathBuf> {
        let src = self.shelf_dir(from).join(id.as_str());
        let dst_dir = self.shelf_dir(to);
        let dst = dst_dir.join(id.as_str());
        if !src.is_dir() {
            return Err(Error::NoSuchRecap(id.to_string()));
        }
        if dst.exists() {
            return Err(Error::AlreadyExists(dst.display().to_string()));
        }
        create_dir(&dst_dir)?;
        fs::rename(&src, &dst).map_err(|e| Error::io(&src, e))?;
        Ok(dst)
    }

    /// Every recap: drafts first, then open, merged, closed; newest PR first
    /// (by `created`, then number) within each. Recaps with
    /// a broken manifest are skipped and reported in the second vector.
    ///
    /// # Errors
    /// [`Error::Io`] if `recaps/` exists but can't be read.
    pub fn list(&self) -> Result<(Vec<RecapSummary>, Vec<String>)> {
        self.list_shelf(Shelf::Active)
    }

    /// Like [`Self::list`], for either shelf.
    ///
    /// # Errors
    /// [`Error::Io`] if the shelf exists but can't be read.
    pub fn list_shelf(&self, which: Shelf) -> Result<(Vec<RecapSummary>, Vec<String>)> {
        let dir = self.shelf_dir(which);
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok((Vec::new(), Vec::new())),
            Err(e) => return Err(Error::io(&dir, e)),
        };
        let mut rows = Vec::new();
        let mut warnings = Vec::new();
        for entry in entries.filter_map(std::result::Result::ok) {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            let Ok(id) = RecapId::new(&name) else {
                continue;
            };
            match read_toml::<RecapManifest>(&path.join(RECAP_TOML)) {
                Ok(Some(m)) => rows.push(RecapSummary {
                    id: id.to_string(),
                    repo: m.repo,
                    number: m.number,
                    title: m.title,
                    state: m.state,
                    ticket: m.ticket,
                    created: m.created,
                    url: m.url,
                }),
                Ok(None) => warnings.push(format!("{name}: no recap.toml")),
                Err(e) => warnings.push(e.to_string()),
            }
        }
        // Drafts, then open, merged, closed (PrState's own order); newest PR
        // first within each.
        rows.sort_by(|a, b| {
            a.state
                .cmp(&b.state)
                .then_with(|| b.created.cmp(&a.created))
                .then_with(|| b.number.cmp(&a.number))
                .then_with(|| a.id.cmp(&b.id))
        });
        Ok((rows, warnings))
    }

    /// Loads recap `id`.
    ///
    /// # Errors
    /// [`Error::NoSuchRecap`] when its directory is missing,
    /// [`Error::ParseToml`] when its manifest is broken. Everything else
    /// degrades to warnings.
    pub fn load(&self, id: &RecapId) -> Result<Recap> {
        self.load_from(Shelf::Active, id)
    }

    /// Loads recap `id` from the active list, or failing that the archive.
    /// For read-only lookups (`show`, `comments`, `merge-check`), so an
    /// archived recap is still readable by id.
    ///
    /// # Errors
    /// As [`Self::load`] (the active list's error when neither has it).
    pub fn load_any(&self, id: &RecapId) -> Result<Recap> {
        self.load(id)
            .or_else(|e| self.load_from(Shelf::Archived, id).map_err(|_| e))
    }

    /// Like [`Self::load`], from either shelf.
    ///
    /// # Errors
    /// As [`Self::load`].
    pub fn load_from(&self, which: Shelf, id: &RecapId) -> Result<Recap> {
        let dir = self.shelf_dir(which).join(id.as_str());
        if !dir.is_dir() {
            return Err(Error::NoSuchRecap(id.to_string()));
        }
        let manifest = read_toml::<RecapManifest>(&dir.join(RECAP_TOML))?
            .ok_or_else(|| Error::NoSuchRecap(id.to_string()))?;
        let mut warnings = Vec::new();
        let summary = read_text(&dir.join(SUMMARY_MD), MAX_FILE_BYTES, &mut warnings);
        let diff = read_text(&dir.join(DIFF_PATCH), MAX_DIFF_BYTES, &mut warnings);
        let diagrams = read_diagrams(&dir.join(DIAGRAMS), &mut warnings);
        Ok(Recap {
            id: id.clone(),
            manifest,
            summary,
            diff,
            diagrams,
            warnings,
        })
    }

    /// Scaffolds a new recap (manifest, diff, summary template, diagrams/).
    ///
    /// # Errors
    /// [`Error::AlreadyExists`] if it exists; [`Error::Io`] on write failure.
    pub fn create(&self, id: &RecapId, manifest: &RecapManifest, diff: &str) -> Result<PathBuf> {
        let dir = self.recap_dir(id);
        if dir.exists() {
            return Err(Error::AlreadyExists(dir.display().to_string()));
        }
        create_dir(&dir.join(DIAGRAMS))?;
        write_atomic(&dir.join(RECAP_TOML), &toml::to_string(manifest)?)?;
        write_atomic(&dir.join(DIFF_PATCH), diff)?;
        write_atomic(&dir.join(SUMMARY_MD), SUMMARY_TEMPLATE)?;
        Ok(dir)
    }

    /// Re-writes an existing recap's manifest and diff. Never touches the
    /// summary or diagrams. A ticket URL already on disk is kept when the
    /// new manifest has none.
    ///
    /// # Errors
    /// [`Error::NoSuchRecap`] or [`Error::Io`].
    pub fn refresh(&self, id: &RecapId, manifest: &RecapManifest, diff: &str) -> Result<PathBuf> {
        let dir = self.recap_dir(id);
        if !dir.is_dir() {
            return Err(Error::NoSuchRecap(id.to_string()));
        }
        let mut manifest = manifest.clone();
        if let Ok(Some(old)) = read_toml::<RecapManifest>(&dir.join(RECAP_TOML)) {
            manifest.ticket_url = manifest.ticket_url.or(old.ticket_url);
            manifest.ticket = manifest.ticket.or(old.ticket);
        }
        write_atomic(&dir.join(RECAP_TOML), &toml::to_string(&manifest)?)?;
        write_atomic(&dir.join(DIFF_PATCH), diff)?;
        Ok(dir)
    }
}

#[cfg(test)]
#[path = "../tests/store.rs"]
mod tests;
