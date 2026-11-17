//! The report store: the directory tree under the store root.
//!
//! ```text
//! <root>/
//!  standups/
//!   2026-10-27/
//!     report.toml          title, since, created
//!     overview.md          the big picture — read first
//!     diagrams/NN-*.txt    big-picture ASCII diagrams
//!     tickets/
//!       ABC-123/
//!         ticket.toml      id, title, url, state, systems, prs
//!         summary.md       the ticket write-up
//!         diagrams/NN-*.txt
//! ```
//!
//! Listing reads only directory names and the small manifests. Every file
//! read is size-capped, and anything malformed degrades to a warning rather
//! than an error — reports are written by agents and must never take the
//! TUI down.

mod export;
mod files;

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::{Error, Result};
pub use export::export_markdown;
use files::{create_dir, read_diagrams, read_text, read_toml, subdirs, write_atomic};

use crate::model::{ReportDate, ReportManifest, StateKind, TicketId, TicketManifest};

/// The directory under the store root that holds the reports.
pub const REPORTS_DIR: &str = "standups";

/// The nearest directory at or above `start` that already contains a
/// `standups/` directory, or `start` itself when none does — git-style
/// discovery: run `beagle standup` anywhere inside a reports checkout and it
/// finds the reports.
#[must_use]
pub fn discover_root(start: &Path) -> PathBuf {
    crate::config::DIRS.discover_root(start)
}

/// Largest file the store will load (2 MiB).
pub const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;

const REPORT_TOML: &str = "report.toml";
const OVERVIEW_MD: &str = "overview.md";
const TICKET_TOML: &str = "ticket.toml";
const SUMMARY_MD: &str = "summary.md";
const DIAGRAMS: &str = "diagrams";
const TICKETS: &str = "tickets";

const OVERVIEW_TEMPLATE: &str = "\
# Since last standup

> Waiting for the write-up…
";

const SUMMARY_TEMPLATE: &str = "\
## TL;DR

## Where this sits in the stack

## The problem

## Why this ticket is needed

## The fix

## Where it stands

## What we need to do
";

/// One row of `beagle standup list`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReportSummary {
    /// The report date (`YYYY-MM-DD`).
    pub date: String,
    /// The optional headline.
    pub title: Option<String>,
    /// The previous standup it covers work since.
    pub since: Option<String>,
    /// How many tickets it has.
    pub tickets: usize,
}

/// A diagram file: its name (without extension) and raw body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagram {
    /// File stem, e.g. `01-request-path`.
    pub name: String,
    /// Raw contents, possibly with ANSI colour codes.
    pub body: String,
}

/// One ticket's write-up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ticket {
    /// The ticket id (also its directory name).
    pub id: TicketId,
    /// Its manifest.
    pub manifest: TicketManifest,
    /// `summary.md`.
    pub summary: String,
    /// `diagrams/*.txt`, in file-name order.
    pub diagrams: Vec<Diagram>,
}

impl Ticket {
    /// The ticket's state bucket.
    #[must_use]
    pub fn state_kind(&self) -> StateKind {
        StateKind::classify(self.manifest.state.as_deref())
    }
}

/// A fully loaded report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// The report date.
    pub date: ReportDate,
    /// `report.toml` (defaults when missing).
    pub manifest: ReportManifest,
    /// `overview.md`.
    pub overview: String,
    /// `diagrams/*.txt`.
    pub diagrams: Vec<Diagram>,
    /// Tickets, most active first (in progress → review → blocked → done → …).
    pub tickets: Vec<Ticket>,
    /// Problems that were skipped over while loading.
    pub warnings: Vec<String>,
}

/// Handle on a store root.
#[derive(Debug, Clone)]
pub struct Store {
    root: PathBuf,
}

impl Store {
    /// A store rooted at `root` (which need not exist yet); reports live in
    /// `root/standups/`.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The store root (the directory containing `standups/`).
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// `<root>/standups/`, where the reports live.
    #[must_use]
    pub fn reports_dir(&self) -> PathBuf {
        self.root.join(REPORTS_DIR)
    }

    /// The directory of the report for `date`.
    #[must_use]
    pub fn report_dir(&self, date: ReportDate) -> PathBuf {
        self.reports_dir().join(date.to_string())
    }

    /// Every report date on disk, newest first. A missing root is an empty
    /// store; directories that aren't dates are ignored.
    ///
    /// # Errors
    /// [`Error::Io`] if the root exists but can't be read.
    pub fn dates(&self) -> Result<Vec<ReportDate>> {
        let dir = self.reports_dir();
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(Error::io(&dir, e)),
        };
        let mut dates: Vec<ReportDate> = entries
            .filter_map(std::result::Result::ok)
            .filter(|e| e.path().is_dir())
            .filter_map(|e| ReportDate::parse(&e.file_name().to_string_lossy()).ok())
            .collect();
        dates.sort_unstable_by(|a, b| b.cmp(a));
        Ok(dates)
    }

    /// The newest report date, if any.
    ///
    /// # Errors
    /// As [`Self::dates`].
    pub fn latest(&self) -> Result<Option<ReportDate>> {
        Ok(self.dates()?.into_iter().next())
    }

    /// The newest report strictly before `date`, if any.
    ///
    /// # Errors
    /// As [`Self::dates`].
    pub fn previous(&self, date: ReportDate) -> Result<Option<ReportDate>> {
        Ok(self.dates()?.into_iter().find(|d| *d < date))
    }

    /// One summary row per report, newest first.
    ///
    /// # Errors
    /// As [`Self::dates`].
    pub fn list(&self) -> Result<Vec<ReportSummary>> {
        Ok(self
            .dates()?
            .into_iter()
            .map(|date| {
                let dir = self.report_dir(date);
                let manifest = read_toml::<ReportManifest>(&dir.join(REPORT_TOML))
                    .ok()
                    .flatten()
                    .unwrap_or_default();
                ReportSummary {
                    date: date.to_string(),
                    title: manifest.title,
                    since: manifest.since,
                    tickets: subdirs(&dir.join(TICKETS)).len(),
                }
            })
            .collect())
    }

    /// Loads the report for `date`.
    ///
    /// # Errors
    /// [`Error::NoSuchReport`] when its directory is missing. Everything
    /// inside it degrades to warnings instead.
    pub fn load(&self, date: ReportDate) -> Result<Report> {
        let dir = self.report_dir(date);
        if !dir.is_dir() {
            return Err(Error::NoSuchReport(date.to_string()));
        }
        let mut warnings = Vec::new();
        let manifest = match read_toml::<ReportManifest>(&dir.join(REPORT_TOML)) {
            Ok(m) => m.unwrap_or_default(),
            Err(e) => {
                warnings.push(e.to_string());
                ReportManifest::default()
            }
        };
        let overview = read_text(&dir.join(OVERVIEW_MD), &mut warnings);
        let diagrams = read_diagrams(&dir.join(DIAGRAMS), &mut warnings);

        let mut tickets = Vec::new();
        for path in subdirs(&dir.join(TICKETS)) {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let Ok(id) = TicketId::new(&name) else {
                warnings.push(format!(
                    "skipped `{}`: not a valid ticket id",
                    path.display()
                ));
                continue;
            };
            let manifest = match read_toml::<TicketManifest>(&path.join(TICKET_TOML)) {
                Ok(Some(m)) => m,
                Ok(None) => TicketManifest {
                    id: id.to_string(),
                    title: "(no ticket.toml yet)".to_owned(),
                    url: None,
                    state: None,
                    systems: Vec::new(),
                    prs: Vec::new(),
                },
                Err(e) => {
                    warnings.push(e.to_string());
                    continue;
                }
            };
            tickets.push(Ticket {
                summary: read_text(&path.join(SUMMARY_MD), &mut warnings),
                diagrams: read_diagrams(&path.join(DIAGRAMS), &mut warnings),
                id,
                manifest,
            });
        }
        tickets.sort_by(|a, b| {
            a.state_kind()
                .cmp(&b.state_kind())
                .then_with(|| a.id.cmp(&b.id))
        });

        Ok(Report {
            date,
            manifest,
            overview,
            diagrams,
            tickets,
            warnings,
        })
    }

    /// Scaffolds a new report directory and returns its path.
    ///
    /// # Errors
    /// [`Error::AlreadyExists`] if the report exists; [`Error::Io`] on
    /// write failures.
    pub fn create_report(&self, date: ReportDate, manifest: &ReportManifest) -> Result<PathBuf> {
        let dir = self.report_dir(date);
        if dir.exists() {
            return Err(Error::AlreadyExists(dir.display().to_string()));
        }
        create_dir(&dir.join(DIAGRAMS))?;
        create_dir(&dir.join(TICKETS))?;
        write_atomic(&dir.join(REPORT_TOML), &toml::to_string(manifest)?)?;
        write_atomic(&dir.join(OVERVIEW_MD), OVERVIEW_TEMPLATE)?;
        Ok(dir)
    }

    /// Scaffolds (or, with `update`, rewrites the manifest of) a ticket
    /// inside the report for `date`. Returns the ticket directory.
    ///
    /// # Errors
    /// [`Error::NoSuchReport`], [`Error::InvalidTicketId`],
    /// [`Error::AlreadyExists`] (without `update`), or [`Error::Io`].
    pub fn upsert_ticket(
        &self,
        date: ReportDate,
        manifest: &TicketManifest,
        update: bool,
    ) -> Result<PathBuf> {
        let report = self.report_dir(date);
        if !report.is_dir() {
            return Err(Error::NoSuchReport(date.to_string()));
        }
        let id = TicketId::new(&manifest.id)?;
        let dir = report.join(TICKETS).join(id.as_str());
        if dir.exists() && !update {
            return Err(Error::AlreadyExists(dir.display().to_string()));
        }
        create_dir(&dir.join(DIAGRAMS))?;
        let manifest = TicketManifest {
            id: id.to_string(),
            ..manifest.clone()
        };
        write_atomic(&dir.join(TICKET_TOML), &toml::to_string(&manifest)?)?;
        let summary = dir.join(SUMMARY_MD);
        if !summary.exists() {
            write_atomic(&summary, SUMMARY_TEMPLATE)?;
        }
        Ok(dir)
    }
}

#[cfg(test)]
#[path = "../tests/store.rs"]
mod tests;
