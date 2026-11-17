//! On-disk data types: report dates, ticket ids, and the two TOML manifests.
//!
//! Everything external is parsed here, at the boundary, into typed values;
//! past this module a [`ReportDate`] or [`TicketId`] is proof its invariant
//! holds.

use std::fmt;

use serde::{Deserialize, Serialize};
use time::macros::format_description;
use time::Date;

use crate::error::{Error, Result};

/// A report's date — also its directory name (`YYYY-MM-DD`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReportDate(Date);

impl ReportDate {
    /// Parses a `YYYY-MM-DD` calendar date.
    ///
    /// # Errors
    /// [`Error::InvalidDate`] for anything else (including `2026-02-30`).
    pub fn parse(s: &str) -> Result<Self> {
        let fmt = format_description!("[year]-[month]-[day]");
        Date::parse(s.trim(), &fmt)
            .map(Self)
            .map_err(|_| Error::InvalidDate(s.to_owned()))
    }

    /// Wraps an already-valid date.
    #[must_use]
    pub fn from_date(date: Date) -> Self {
        Self(date)
    }

    /// A short label, e.g. `Wed 10-21`.
    #[must_use]
    pub fn short(self) -> String {
        let day = self.weekday();
        let abbrev: String = day.chars().take(3).collect();
        format!(
            "{abbrev} {:02}-{:02}",
            u8::from(self.0.month()),
            self.0.day()
        )
    }

    /// The day of the week, e.g. `Monday`.
    #[must_use]
    pub fn weekday(self) -> String {
        self.0.weekday().to_string()
    }
}

impl fmt::Display for ReportDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (y, m, d) = (self.0.year(), u8::from(self.0.month()), self.0.day());
        write!(f, "{y:04}-{m:02}-{d:02}")
    }
}

/// A tracker ticket id such as `ABC-123`. Stored upper-cased; doubles as the
/// ticket's directory name, so it must be filesystem-safe.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TicketId(String);

impl TicketId {
    /// Validates and upper-cases a ticket id.
    ///
    /// # Errors
    /// [`Error::InvalidTicketId`] unless it is 1..=32 chars of
    /// `[A-Za-z0-9_-]`.
    pub fn new(s: &str) -> Result<Self> {
        let ok = (1..=32).contains(&s.len())
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        if ok {
            Ok(Self(s.to_ascii_uppercase()))
        } else {
            Err(Error::InvalidTicketId(s.to_owned()))
        }
    }

    /// The id as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TicketId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// `report.toml`: the report's small manifest.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReportManifest {
    /// Optional one-line headline for the day.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The previous standup this report covers work since (`YYYY-MM-DD`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
    /// When the report was scaffolded (RFC 3339).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
}

/// `tickets/<ID>/ticket.toml`: one ticket's manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TicketManifest {
    /// The ticket id, e.g. `ABC-123`.
    pub id: String,
    /// The ticket title, as in the tracker.
    pub title: String,
    /// The ticket's URL in the tracker (`o` in the TUI opens it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// The ticket's workflow state, as the tracker names it (`In Progress`, …).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    /// The parts of the stack it touches (`api-gateway`, `scheduler`, …).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub systems: Vec<String>,
    /// Pull request URLs for the work.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prs: Vec<String>,
}

/// A coarse bucket for a free-form tracker state, for colour and ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum StateKind {
    /// Work is happening.
    Progress,
    /// Waiting on code review.
    Review,
    /// Stuck on something.
    Blocked,
    /// Shipped / closed.
    Done,
    /// Not started.
    Todo,
    /// Cancelled, duplicate, or unrecognised.
    Other,
}

impl StateKind {
    /// Buckets a tracker workflow-state name. Unknown names are [`Self::Other`].
    #[must_use]
    pub fn classify(state: Option<&str>) -> Self {
        let Some(s) = state else {
            return Self::Other;
        };
        let s = s.to_ascii_lowercase();
        if s.contains("review") || s.contains("qa") {
            Self::Review
        } else if s.contains("progress") || s.contains("doing") || s.contains("started") {
            Self::Progress
        } else if s.contains("block") || s.contains("hold") {
            Self::Blocked
        } else if s.contains("done")
            || s.contains("merged")
            || s.contains("complete")
            || s.contains("deployed")
            || s.contains("closed")
        {
            Self::Done
        } else if s.contains("todo")
            || s.contains("backlog")
            || s.contains("triage")
            || s.contains("planned")
        {
            Self::Todo
        } else {
            Self::Other
        }
    }
}

#[cfg(test)]
#[path = "tests/model.rs"]
mod tests;
