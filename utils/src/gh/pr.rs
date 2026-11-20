//! A pull request's state, as `gh` reports it.

use std::fmt;

/// Where a pull request stands. Parsed at the boundary from GitHub's
/// `state` (`OPEN` / `MERGED` / `CLOSED`) plus `isDraft`, and serialized as
/// a lowercase word (`"draft"`).
///
/// The declaration order is the natural list order: drafts, open, merged,
/// closed (`Ord` is derived from it).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum PrState {
    /// Open, but marked draft.
    Draft,
    /// Open and ready for review.
    Open,
    /// Merged: the change has landed.
    Merged,
    /// Closed without merging.
    Closed,
}

impl PrState {
    /// Every state, in list order.
    pub const ALL: [Self; 4] = [Self::Draft, Self::Open, Self::Merged, Self::Closed];

    /// Maps GitHub's `state` and `isDraft` fields to a state. `None` for
    /// anything GitHub doesn't document.
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

    /// The lowercase word: `draft`, `open`, `merged` or `closed`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Open => "open",
            Self::Merged => "merged",
            Self::Closed => "closed",
        }
    }

    /// Whether the PR is still open (draft or ready).
    #[must_use]
    pub fn is_open(self) -> bool {
        matches!(self, Self::Draft | Self::Open)
    }
}

impl fmt::Display for PrState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Parses `gh pr view --json state,isDraft` output (extra fields are
/// ignored). `None` when it isn't JSON or the state is unknown.
#[must_use]
pub fn parse_pr_state(json: &str) -> Option<PrState> {
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Raw {
        state: String,
        #[serde(default)]
        is_draft: bool,
    }
    let raw: Raw = serde_json::from_str(json).ok()?;
    PrState::from_github(&raw.state, raw.is_draft)
}
