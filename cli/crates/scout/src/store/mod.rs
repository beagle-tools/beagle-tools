//! The on-disk store: one directory per project, plain files, no database.
//!
//! ```text
//! <root>/                      # --root, else $SCOUT_ROOT, else ~/.scout
//!   rate-limit-the-api/        # project id: a slug
//!     project.md               # goal, problem, how the stack works, done-when
//!     log.md                   # timestamped activity, appended by `beagle scout log`
//!     tasks/
//!       01-map-request-path.md # NN-slug.md: NN orders the tasks
//!       02-add-allowlist.md
//! ```
//!
//! Every mutation also appends a line to the project's `log.md`, so the log
//! is a complete record of what happened even when an agent forgets to
//! narrate. Writes are atomic (temp file + rename).

use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write as _};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::de::DeserializeOwned;
use serde::Serialize;
use time::macros::format_description;
use time::OffsetDateTime;

use crate::error::{Error, Result};
use crate::frontmatter;
use crate::markdown;
use crate::model::{ProjectMeta, ProjectStatus, Slug, TaskMeta, TaskStatus};

/// The project's goal document.
pub const PROJECT_FILE: &str = "project.md";
/// The directory holding the project's task files.
pub const TASKS_DIR: &str = "tasks";
/// The project's activity log.
pub const LOG_FILE: &str = "log.md";

/// Hidden index: one file per attached session, listing its projects.
pub const ATTACHED_DIR: &str = ".attached";

/// Files above this size are refused rather than read.
const MAX_FILE_BYTES: u64 = 4 * 1024 * 1024;

/// The skeleton every new `project.md` starts from. Each section is written
/// for someone who does not know the system — see the `/scout` skill.
pub const PROJECT_TEMPLATE: &str = "
## Goal

<One or two sentences: what will be true when this project is finished.>

## The problem

<What is wrong or missing today, who it hurts, and why it matters now.>

## How this part of the stack works

<Explain the pieces involved as if to someone new to the system: what each one
is for, how a request or a piece of data flows through them, and where this
project's change lands. Plain words first; a small ASCII diagram helps.>

## The plan

<The approach in a few sentences. The tasks are the steps, in order.>

## Done when

- [ ] <An observable outcome that proves the goal is met>
";

/// The skeleton every new task file starts from.
pub const TASK_TEMPLATE: &str = "
## Context

<What the piece of the system this task touches is, in two or three plain
sentences, and where it sits in the stack.>

## Objective

<Exactly what changes and where: repo, file, resource, command.>

## Why this moves the goal

<How finishing this task gets the project closer to its goal.>

## Steps

- [ ] <step>

## Done when

- [ ] <a check anyone could run to confirm the task is finished>

## Outcome

<Filled in when the task closes: what was done, links to PRs, surprises.>

## Next steps

<Filled in when the task closes or work pauses: what should happen next, and
anything the next task (or the next session) needs to know to pick it up.>
";

/// The task sections shown on the TUI's Outcome tab: the handoff from this
/// task to whatever comes next.
pub const HANDOFF_SECTIONS: [&str; 2] = ["Outcome", "Next steps"];

/// Whether section text is empty or still an unfilled `<template
/// placeholder>`.
#[must_use]
pub fn is_placeholder(text: &str) -> bool {
    let text = text.trim();
    text.is_empty() || (text.starts_with('<') && text.ends_with('>'))
}

/// The store rooted at one directory.
#[derive(Debug, Clone)]
pub struct Store {
    root: PathBuf,
}

/// A loaded project and its tasks.
#[derive(Debug, Clone)]
pub struct Project {
    /// The directory name.
    pub id: Slug,
    /// `project.md` frontmatter.
    pub meta: ProjectMeta,
    /// The project directory.
    pub dir: PathBuf,
    /// Tasks in order (by number).
    pub tasks: Vec<Task>,
    /// The newest modification time across the project's files.
    pub last_activity: Option<SystemTime>,
}

/// A loaded task.
#[derive(Debug, Clone)]
pub struct Task {
    /// The file stem, `NN-slug`.
    pub id: String,
    /// `NN`: the task's place in the order.
    pub number: u32,
    /// The slug after `NN-`.
    pub slug: String,
    /// The task's frontmatter.
    pub meta: TaskMeta,
    /// The task file.
    pub path: PathBuf,
    /// The task file's modification time.
    pub modified: Option<SystemTime>,
    /// `(checked, total)` checkboxes in the task body.
    pub checklist: (usize, usize),
}

impl Task {
    /// The task's `## Next steps` section, if written (not a placeholder).
    #[must_use]
    pub fn next_steps(&self) -> Option<String> {
        let source = read_capped(&self.path).ok()?;
        let (_, picked) = markdown::split_sections(frontmatter::body(&source), &["Next steps"]);
        let text: String = picked
            .lines()
            .skip(1) // the heading itself
            .collect::<Vec<_>>()
            .join("\n");
        let text = text.trim();
        (!is_placeholder(text)).then(|| text.to_owned())
    }
}

/// Every project under the root, plus files that failed to load.
#[derive(Debug, Default)]
pub struct Listing {
    /// Loaded projects: open ones first, then most recently active.
    pub projects: Vec<Project>,
    /// One message per file that could not load. Never fatal.
    pub warnings: Vec<String>,
}

impl Project {
    /// The `project.md` path.
    #[must_use]
    pub fn file(&self) -> PathBuf {
        self.dir.join(PROJECT_FILE)
    }

    /// The `log.md` path.
    #[must_use]
    pub fn log_file(&self) -> PathBuf {
        self.dir.join(LOG_FILE)
    }

    /// How many tasks are done or skipped.
    #[must_use]
    pub fn closed_count(&self) -> usize {
        self.tasks
            .iter()
            .filter(|t| t.meta.status.is_closed())
            .count()
    }

    /// Tasks an agent is working on right now.
    pub fn working(&self) -> impl Iterator<Item = &Task> {
        self.tasks
            .iter()
            .filter(|t| t.meta.status == TaskStatus::Working)
    }

    /// The first task not yet started.
    #[must_use]
    pub fn next_todo(&self) -> Option<&Task> {
        self.tasks
            .iter()
            .find(|t| t.meta.status == TaskStatus::Todo)
    }

    /// Finds a task by number (`3`, `03`), full id (`03-add-allowlist`), or
    /// slug (`add-allowlist`).
    #[must_use]
    pub fn resolve_task(&self, reference: &str) -> Option<&Task> {
        let reference = reference.trim();
        if !reference.is_empty() && reference.bytes().all(|b| b.is_ascii_digit()) {
            let number: u32 = reference.parse().ok()?;
            return self.tasks.iter().find(|t| t.number == number);
        }
        self.tasks
            .iter()
            .find(|t| t.id == reference)
            .or_else(|| self.tasks.iter().find(|t| t.slug == reference))
    }

    fn find_task(&self, reference: &str) -> Result<&Task> {
        self.resolve_task(reference)
            .ok_or_else(|| Error::TaskNotFound {
                project: self.id.to_string(),
                task: reference.to_owned(),
            })
    }

    /// The most relevant `## Next steps`, with the task it came from: the
    /// task being worked if it has some (it paused mid-way), else the most
    /// recently updated closed task. This is what an agent picking up the
    /// project reads first.
    #[must_use]
    pub fn handoff(&self) -> Option<(&Task, String)> {
        fn with_steps(t: &Task) -> Option<(&Task, String)> {
            t.next_steps().map(|steps| (t, steps))
        }
        self.working().find_map(with_steps).or_else(|| {
            let mut closed: Vec<&Task> = self
                .tasks
                .iter()
                .filter(|t| t.meta.status.is_closed())
                .collect();
            closed.sort_by_key(|t| std::cmp::Reverse(t.meta.updated.unwrap_or(t.meta.created)));
            closed.into_iter().find_map(with_steps)
        })
    }

    /// The project as one row of `beagle scout list --json` (`beagle
    /// serve` answers the same row, so both views always agree).
    #[must_use]
    pub fn list_json(&self) -> serde_json::Value {
        serde_json::json!({
            "id": self.id.as_str(),
            "title": self.meta.title,
            "status": self.meta.status.as_str(),
            "tasks_closed": self.closed_count(),
            "tasks_total": self.tasks.len(),
            "working": self.working().map(|t| t.id.as_str()).collect::<Vec<_>>(),
            "links": self.meta.links,
            "dir": self.dir,
        })
    }

    /// The first paragraph of the `## Goal` section, flattened to one line.
    #[must_use]
    pub fn goal(&self) -> Option<String> {
        let source = read_capped(&self.file()).ok()?;
        section_first_paragraph(frontmatter::body(&source), "Goal")
    }
}

impl Store {
    /// A store rooted at `root`. Nothing is touched until a method runs.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The root directory.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Loads every project. A missing root is an empty listing; a broken
    /// project or task becomes a warning, never an error.
    ///
    /// # Errors
    /// [`Error::Io`] if the root exists but cannot be listed.
    pub fn list(&self) -> Result<Listing> {
        let mut listing = Listing::default();
        let entries = match fs::read_dir(&self.root) {
            Ok(entries) => entries,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(listing),
            Err(e) => return Err(Error::io(&self.root)(e)),
        };
        for entry in entries {
            let path = entry.map_err(Error::io(&self.root))?.path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if name.starts_with('.') || !path.join(PROJECT_FILE).is_file() {
                continue;
            }
            let Ok(id) = Slug::parse(name) else {
                listing
                    .warnings
                    .push(format!("{}: not a valid project id", path.display()));
                continue;
            };
            match load_dir(id, path) {
                Ok((project, warnings)) => {
                    listing.projects.push(project);
                    listing.warnings.extend(warnings);
                }
                Err(e) => listing.warnings.push(e.to_string()),
            }
        }
        listing.projects.sort_by(|a, b| {
            b.meta
                .status
                .is_open()
                .cmp(&a.meta.status.is_open())
                .then_with(|| b.last_activity.cmp(&a.last_activity))
                .then_with(|| a.id.cmp(&b.id))
        });
        Ok(listing)
    }

    /// Loads one project, with a warning per task file that failed to load.
    ///
    /// # Errors
    /// [`Error::InvalidSlug`], [`Error::ProjectNotFound`], or the project
    /// file's own read/parse error.
    pub fn load_project(&self, id: &str) -> Result<(Project, Vec<String>)> {
        let slug = Slug::parse(id)?;
        let dir = self.root.join(slug.as_str());
        if !dir.join(PROJECT_FILE).is_file() {
            return Err(Error::ProjectNotFound(id.to_owned()));
        }
        load_dir(slug, dir)
    }

    /// Loads one project, ignoring task files that failed to load.
    ///
    /// # Errors
    /// As [`Store::load_project`].
    pub fn project(&self, id: &str) -> Result<Project> {
        self.load_project(id).map(|(project, _)| project)
    }
}

mod files;
mod mutate;

use files::{
    append_line, load_dir, load_task, log_line, now, now_from, section_first_paragraph, update_doc,
    write_atomic,
};
pub use files::{modified, read_capped, split_task_stem};
pub use mutate::AutoPaused;

#[cfg(test)]
mod tests;
