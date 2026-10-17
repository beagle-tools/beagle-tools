//! [`TicketPlatform`]: classifying attached ticket references.

/// A ticket-tracking platform beagle can attach and sync.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TicketPlatform {
    /// A GitHub issue (`github.com/<owner>/<repo>/issues/<n>`).
    GitHub,
    /// A Linear issue (`linear.app/...` or a bare `ABC-123` id).
    Linear,
}

impl TicketPlatform {
    /// Classifies a ticket URL/id by platform. `None` when it matches
    /// neither — the shape a `beagle ticket add` should reject.
    #[must_use]
    pub fn of(reference: &str) -> Option<Self> {
        let r = reference.trim();
        if r.contains("linear.app/") {
            return Some(Self::Linear);
        }
        if r.contains("github.com/") && r.contains("/issues/") {
            return Some(Self::GitHub);
        }
        // A bare Linear id like `ENG-123` (letters, dash, digits).
        let mut parts = r.splitn(2, '-');
        if let (Some(team), Some(num)) = (parts.next(), parts.next()) {
            if !team.is_empty()
                && team.chars().all(|c| c.is_ascii_alphabetic())
                && !num.is_empty()
                && num.chars().all(|c| c.is_ascii_digit())
            {
                return Some(Self::Linear);
            }
        }
        None
    }

    /// Human-readable name.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::GitHub => "github",
            Self::Linear => "linear",
        }
    }
}
