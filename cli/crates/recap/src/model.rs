//! On-disk data types: recap ids, PR references, and the `recap.toml`
//! manifest. External data is parsed here, at the boundary.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// A recap's id — also its directory name: `<repo>-<number>`, lowercase,
/// e.g. `widgets-42`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RecapId(String);

impl RecapId {
    /// Validates an id: 1..=96 chars of `[a-z0-9-]`.
    ///
    /// # Errors
    /// [`Error::InvalidId`] otherwise.
    pub fn new(s: &str) -> Result<Self> {
        let ok = (1..=96).contains(&s.len())
            && s.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        if ok {
            Ok(Self(s.to_owned()))
        } else {
            Err(Error::InvalidId(s.to_owned()))
        }
    }

    /// The id for a PR: repo name (lowercased, unsafe chars → `-`) + number.
    #[must_use]
    pub fn for_pr(pr: &PrRef) -> Self {
        let repo: String = pr
            .repo
            .chars()
            .map(|c| {
                let c = c.to_ascii_lowercase();
                if c.is_ascii_lowercase() || c.is_ascii_digit() {
                    c
                } else {
                    '-'
                }
            })
            .collect();
        Self(format!("{}-{}", repo.trim_matches('-'), pr.number))
    }

    /// The id as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RecapId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A pull request on GitHub.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrRef {
    /// Owner (org or user).
    pub owner: String,
    /// Repository name.
    pub repo: String,
    /// PR number.
    pub number: u64,
}

impl PrRef {
    /// Parses `https://github.com/<owner>/<repo>/pull/<n>[/…]` or
    /// `<owner>/<repo>#<n>`.
    ///
    /// # Errors
    /// [`Error::InvalidPr`] for anything else.
    pub fn parse(s: &str) -> Result<Self> {
        let bad = || Error::InvalidPr(s.to_owned());
        let t = s.trim();
        let (owner, repo, number) = if let Some(rest) = t
            .strip_prefix("https://github.com/")
            .or_else(|| t.strip_prefix("http://github.com/"))
            .or_else(|| t.strip_prefix("github.com/"))
        {
            let mut parts = rest.split('/');
            let owner = parts.next().ok_or_else(bad)?;
            let repo = parts.next().ok_or_else(bad)?;
            if parts.next() != Some("pull") {
                return Err(bad());
            }
            let n = parts.next().ok_or_else(bad)?;
            let n = n.split(['?', '#']).next().unwrap_or(n);
            (owner, repo, n)
        } else {
            let (path, n) = t.split_once('#').ok_or_else(bad)?;
            let (owner, repo) = path.split_once('/').ok_or_else(bad)?;
            (owner, repo, n)
        };
        let number: u64 = number.parse().map_err(|_| bad())?;
        let valid = |p: &str| {
            !p.is_empty()
                && p.chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        };
        if !valid(owner) || !valid(repo) || number == 0 {
            return Err(bad());
        }
        Ok(Self {
            owner: owner.to_owned(),
            repo: repo.to_owned(),
            number,
        })
    }

    /// `owner/repo`.
    #[must_use]
    pub fn full_repo(&self) -> String {
        format!("{}/{}", self.owner, self.repo)
    }
}

impl fmt::Display for PrRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}#{}", self.owner, self.repo, self.number)
    }
}

/// `recap.toml`: the PR's metadata, as pulled from GitHub (plus the ticket).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecapManifest {
    /// `owner/repo`.
    pub repo: String,
    /// PR number.
    pub number: u64,
    /// PR title.
    pub title: String,
    /// PR URL (`o` in the TUI opens it).
    pub url: String,
    /// `draft`, `open`, `merged` or `closed`.
    pub state: PrState,
    /// PR author login.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// Head branch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// Base branch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    /// Ticket id, e.g. `ABC-123` (taken from the title prefix when present).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ticket: Option<String>,
    /// Ticket URL (`t` in the TUI opens it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ticket_url: Option<String>,
    /// Lines added.
    #[serde(default)]
    pub additions: u64,
    /// Lines deleted.
    #[serde(default)]
    pub deletions: u64,
    /// Files changed.
    #[serde(default)]
    pub files: u64,
    /// When the PR was opened (RFC 3339, from GitHub).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
    /// When the PR last changed (RFC 3339, from GitHub).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated: Option<String>,
    /// When the PR merged (RFC 3339, from GitHub). Merged recaps are
    /// archived 24 hours after this.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merged_at: Option<String>,
}

/// Seconds since the Unix epoch for an RFC 3339 timestamp as GitHub writes
/// them (`2026-10-27T18:44:40Z`, optionally with fractional seconds or a
/// `±HH:MM` offset). `None` for anything else.
#[must_use]
pub fn unix_seconds(ts: &str) -> Option<i64> {
    let ts = ts.trim();
    let num = |s: &str| s.parse::<i64>().ok();
    let (date, rest) = ts.split_once('T')?;
    let mut date_parts = date.split('-');
    let (year, mo, da) = (
        num(date_parts.next()?)?,
        num(date_parts.next()?)?,
        num(date_parts.next()?)?,
    );
    // Split the time from its zone: `Z`, or the last `+`/`-`.
    let (time, offset) =
        if let Some(clock) = rest.strip_suffix('Z').or_else(|| rest.strip_suffix('z')) {
            (clock, 0)
        } else {
            let at = rest.rfind(['+', '-'])?;
            let (clock, zone) = rest.split_at(at);
            let sign = if zone.starts_with('-') { -1 } else { 1 };
            let (zh, zm) = zone[1..].split_once(':')?;
            (clock, sign * (num(zh)? * 3600 + num(zm)? * 60))
        };
    let time = time.split('.').next()?;
    let mut time_parts = time.split(':');
    let (hour, mi, sec) = (
        num(time_parts.next()?)?,
        num(time_parts.next()?)?,
        num(time_parts.next()?)?,
    );
    if !(1..=12).contains(&mo) || !(1..=31).contains(&da) || hour > 23 || mi > 59 || sec > 60 {
        return None;
    }
    // Days from the civil date (Howard Hinnant's algorithm).
    let yy = if mo <= 2 { year - 1 } else { year };
    let era = yy.div_euclid(400);
    let yoe = yy - era * 400;
    let mp = (mo + 9) % 12;
    let doy = (153 * mp + 2) / 5 + da - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400 + hour * 3600 + mi * 60 + sec - offset)
}

/// A PR's state. Parsed at the boundary — from GitHub's `state` + `isDraft`
/// fields, and as a lowercase enum in `recap.toml` — never from a label.
///
/// The declaration order **is** the PR list's order: drafts, open, merged,
/// closed (`Ord` is derived from it).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PrState {
    /// Open, but marked draft.
    Draft,
    /// Open, ready for review.
    Open,
    /// Merged.
    Merged,
    /// Closed without merging.
    Closed,
}

impl PrState {
    /// Every state, in list order.
    pub const ALL: [Self; 4] = [Self::Draft, Self::Open, Self::Merged, Self::Closed];

    /// Maps GitHub's PR fields (`state`: `OPEN` / `MERGED` / `CLOSED`, plus
    /// `isDraft`) to a state. `None` for anything GitHub doesn't document.
    #[must_use]
    pub fn from_github(state: &str, is_draft: bool) -> Option<Self> {
        match (state, is_draft) {
            ("OPEN", true) => Some(Self::Draft),
            ("OPEN", false) => Some(Self::Open),
            ("MERGED", _) => Some(Self::Merged),
            ("CLOSED", _) => Some(Self::Closed),
            _ => None,
        }
    }

    /// Upper-case label for the TUI.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Draft => "DRAFT",
            Self::Open => "OPEN",
            Self::Merged => "MERGED",
            Self::Closed => "CLOSED",
        }
    }

    /// Group heading for the PR list.
    #[must_use]
    pub fn heading(self) -> &'static str {
        match self {
            Self::Draft => "DRAFTS",
            Self::Open => "OPEN",
            Self::Merged => "MERGED",
            Self::Closed => "CLOSED",
        }
    }
}

impl fmt::Display for PrState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Draft => "draft",
            Self::Open => "open",
            Self::Merged => "merged",
            Self::Closed => "closed",
        })
    }
}

/// A ticket id at the start of a PR title (`ABC-123: …`, `OPS-4177 …`).
#[must_use]
pub fn ticket_from_title(title: &str) -> Option<String> {
    let head: String = title
        .trim_start()
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect();
    let (prefix, digits) = head.split_once('-')?;
    let ok = !prefix.is_empty()
        && prefix.chars().all(|c| c.is_ascii_uppercase())
        && !digits.is_empty()
        && digits.chars().all(|c| c.is_ascii_digit());
    ok.then_some(head)
}

#[cfg(test)]
#[path = "tests/model.rs"]
mod tests;
