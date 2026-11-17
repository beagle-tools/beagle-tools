//! The on-disk data model: ids, statuses, and the frontmatter of project and
//! task files.
//!
//! The frontmatter structs reject unknown fields, so a typo in a hand-edited
//! file is reported instead of silently ignored.

use std::fmt;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::error::{Error, Result};

/// The longest id scout accepts.
pub const MAX_SLUG_LEN: usize = 64;

/// A validated id: lowercase `[a-z0-9-]`, 1–64 characters, no `-` at either
/// end. Used for project directory names and the slug part of task files.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Slug(String);

impl Slug {
    /// Validates `raw` as a slug.
    ///
    /// # Errors
    /// [`Error::InvalidSlug`] when `raw` breaks the rules above.
    pub fn parse(raw: &str) -> Result<Self> {
        let valid = !raw.is_empty()
            && raw.len() <= MAX_SLUG_LEN
            && raw
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            && !raw.starts_with('-')
            && !raw.ends_with('-');
        if valid {
            Ok(Self(raw.to_owned()))
        } else {
            Err(Error::InvalidSlug(raw.to_owned()))
        }
    }

    /// The slug as a string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Slug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Where a project is in its life.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProjectStatus {
    /// Goal and tasks are being written; no work has started.
    Planning,
    /// Tasks are being worked.
    Active,
    /// Parked on purpose; will resume.
    Paused,
    /// The goal was reached.
    Done,
    /// Abandoned; the goal no longer matters.
    Dropped,
}

impl ProjectStatus {
    /// Every status, in lifecycle order.
    pub const ALL: [Self; 5] = [
        Self::Planning,
        Self::Active,
        Self::Paused,
        Self::Done,
        Self::Dropped,
    ];

    /// The on-disk spelling.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Planning => "planning",
            Self::Active => "active",
            Self::Paused => "paused",
            Self::Done => "done",
            Self::Dropped => "dropped",
        }
    }

    /// Parses the on-disk spelling.
    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.as_str() == raw)
    }

    /// Whether the project still has work ahead of it.
    #[must_use]
    pub fn is_open(self) -> bool {
        matches!(self, Self::Planning | Self::Active | Self::Paused)
    }
}

/// Where a task is in its life.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TaskStatus {
    /// Not started.
    Todo,
    /// An agent is on it right now.
    Working,
    /// Started, but nobody is on it right now (set by hand, or automatically
    /// after `[auto] pause_after` of quiet).
    Paused,
    /// Cannot proceed until something outside the task changes.
    Blocked,
    /// Finished; its objective was met.
    Done,
    /// Deliberately not done (replanned away), kept for the record.
    Skipped,
}

impl TaskStatus {
    /// Every status, in lifecycle order.
    pub const ALL: [Self; 6] = [
        Self::Todo,
        Self::Working,
        Self::Paused,
        Self::Blocked,
        Self::Done,
        Self::Skipped,
    ];

    /// The on-disk spelling.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Todo => "todo",
            Self::Working => "working",
            Self::Paused => "paused",
            Self::Blocked => "blocked",
            Self::Done => "done",
            Self::Skipped => "skipped",
        }
    }

    /// Parses the on-disk spelling.
    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.as_str() == raw)
    }

    /// Whether the task needs no more work (done or skipped).
    #[must_use]
    pub fn is_closed(self) -> bool {
        matches!(self, Self::Done | Self::Skipped)
    }
}

/// The frontmatter of `project.md`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectMeta {
    /// One-line name of the goal.
    pub title: String,
    /// Lifecycle status.
    pub status: ProjectStatus,
    /// When the project was created (RFC 3339, quoted).
    #[serde(with = "time::serde::rfc3339")]
    pub created: OffsetDateTime,
    /// When the frontmatter last changed.
    #[serde(
        default,
        with = "time::serde::rfc3339::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub updated: Option<OffsetDateTime>,
    /// Related tickets, PRs, docs: ticket ids, URLs, `backlog:TASK-12`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<String>,
}

/// The frontmatter of a task file (`tasks/NN-slug.md`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskMeta {
    /// One-line, precise statement of the task.
    pub title: String,
    /// Lifecycle status.
    pub status: TaskStatus,
    /// When the task was created (RFC 3339, quoted).
    #[serde(with = "time::serde::rfc3339")]
    pub created: OffsetDateTime,
    /// When the frontmatter last changed.
    #[serde(
        default,
        with = "time::serde::rfc3339::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub updated: Option<OffsetDateTime>,
    /// Related tickets, PRs, docs: ticket ids, URLs, `backlog:TASK-12`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<String>,
    /// Claude Code sessions attached to this task (`beagle scout task attach`),
    /// shown on the TUI's Session tab.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sessions: Vec<String>,
}

#[cfg(test)]
// Tests fail loudly on setup errors; unwrap/expect/panic is the clearest way.
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn slug_rules() {
        assert!(Slug::parse("rate-limit-the-api").is_ok());
        assert!(Slug::parse("a").is_ok());
        assert!(Slug::parse("2026-11-12-x").is_ok());
        for bad in ["", "-a", "a-", "Upper", "under_score", "sp ace", "dot.s"] {
            assert!(Slug::parse(bad).is_err(), "{bad:?} should be rejected");
        }
        assert!(Slug::parse(&"a".repeat(64)).is_ok());
        assert!(Slug::parse(&"a".repeat(65)).is_err());
    }

    #[test]
    fn statuses_round_trip() {
        for s in ProjectStatus::ALL {
            assert_eq!(ProjectStatus::parse(s.as_str()), Some(s));
        }
        for s in TaskStatus::ALL {
            assert_eq!(TaskStatus::parse(s.as_str()), Some(s));
        }
        assert_eq!(TaskStatus::parse("in-progress"), None);
    }

    #[test]
    fn closed_and_open() {
        assert!(TaskStatus::Done.is_closed());
        assert!(TaskStatus::Skipped.is_closed());
        assert!(!TaskStatus::Blocked.is_closed());
        assert!(ProjectStatus::Paused.is_open());
        assert!(!ProjectStatus::Done.is_open());
    }
}
