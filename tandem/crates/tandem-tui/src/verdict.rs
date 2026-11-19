//! The four verdicts a review ends on. The verdict is the first line of
//! the stored summary, bold, so it is the first thing you read; the list
//! shows it as a badge.

/// How mergeable the reviewer found the PR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Mergeable,
    WithSuggestions,
    WithUnknowns,
    NotMergeable,
}

impl Verdict {
    pub const ALL: [Self; 4] = [
        Self::Mergeable,
        Self::WithSuggestions,
        Self::WithUnknowns,
        Self::NotMergeable,
    ];

    /// The key agents write in a `tandem_verdict` line.
    pub fn key(self) -> &'static str {
        match self {
            Self::Mergeable => "mergeable",
            Self::WithSuggestions => "mergeable-with-suggestions",
            Self::WithUnknowns => "mergeable-with-unknowns",
            Self::NotMergeable => "not-mergeable",
        }
    }

    /// The words shown in the TUI.
    pub fn label(self) -> &'static str {
        match self {
            Self::Mergeable => "Mergeable",
            Self::WithSuggestions => "Mergeable with suggestions",
            Self::WithUnknowns => "Mergeable but there are some unknowns",
            Self::NotMergeable => "Not mergeable, there are issues",
        }
    }

    /// A short badge for the PR list.
    pub fn badge(self) -> &'static str {
        match self {
            Self::Mergeable => "merge",
            Self::WithSuggestions => "merge+",
            Self::WithUnknowns => "merge?",
            Self::NotMergeable => "block",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.key() == key)
    }

    /// The verdict line at the top of a summary.
    pub fn heading(self, reason: &str) -> String {
        format!("**{}** — {}", self.label(), reason.trim())
    }

    /// Reads the verdict back from a stored summary's first line.
    pub fn from_summary(body: &str) -> Option<Self> {
        let first = body.lines().next()?;
        // Longest label first: "Mergeable" is a prefix of two others.
        let mut all = Self::ALL;
        all.sort_by_key(|v| std::cmp::Reverse(v.label().len()));
        all.into_iter()
            .find(|v| first.starts_with(&format!("**{}**", v.label())))
    }

    /// `body` with its verdict line replaced (or added) by `heading`.
    pub fn replace_heading(body: &str, heading: &str) -> String {
        let rest = if Self::from_summary(body).is_some() {
            body.split_once('\n').map_or("", |(_, rest)| rest)
        } else {
            body
        };
        format!("{heading}\n\n{}", rest.trim_start_matches('\n'))
    }
}

#[cfg(test)]
// Tests panic on a broken fixture; that is the failure mode we want.
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn keys_round_trip_and_headings_read_back() {
        for v in Verdict::ALL {
            assert_eq!(Verdict::from_key(v.key()), Some(v));
            assert_eq!(Verdict::from_summary(&v.heading("why")), Some(v));
        }
        assert_eq!(Verdict::from_key("lgtm"), None);
        assert_eq!(Verdict::from_summary("no verdict"), None);
    }

    #[test]
    fn replacing_the_heading_keeps_the_summary() {
        let first = Verdict::NotMergeable.heading("a bug");
        let body = format!("{first}\n\n## What is the problem\nX");
        let next = Verdict::replace_heading(&body, &Verdict::Mergeable.heading("fixed"));
        assert!(next.starts_with("**Mergeable** — fixed\n\n## What is the problem"));
        let fresh = Verdict::replace_heading("## What\nY", &Verdict::Mergeable.heading("ok"));
        assert!(fresh.starts_with("**Mergeable** — ok\n\n## What"));
    }
}
