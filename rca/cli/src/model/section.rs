//! [`SectionKind`]: the fixed set of markdown section files in a workspace.

/// The markdown sections every workspace can contain. Each maps to one file
/// in the workspace directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SectionKind {
    /// What broke, in three sentences. The tab a responder reads first.
    Summary,
    /// What happened when, as observed in telemetry.
    Timeline,
    /// Why it broke: the causal chain down to the root.
    RootCause,
    /// Who and what was affected, quantified.
    Impact,
    /// How to fix it: immediate mitigation and durable fix.
    Remediation,
    /// Optional: the test commands to run to validate the RCA's theory
    /// (bash in code fences). Opt-in — not scaffolded; its tab appears
    /// only when `commands.md` exists.
    Commands,
    /// The verification checklist: concrete, checkable predictions of what
    /// "fixed" looks like, written **during** the investigation so the
    /// `final-review` phase knows exactly what to confirm once the fix
    /// PR merges.
    FinalReview,
    /// Raw evidence, queries, links, and loose ends.
    Notes,
    /// Append-only investigation log: what the investigator did, when.
    /// The live "watch the agent think" stream.
    Log,
}

impl SectionKind {
    /// Every section, in tab order. Includes the optional `Commands`
    /// section so reads, exports, and mtime tracking treat it uniformly;
    /// [`is_optional`](Self::is_optional) marks the ones that are opt-in
    /// (not scaffolded, tab hidden until the file exists).
    pub const ALL: [Self; 9] = [
        Self::Summary,
        Self::Timeline,
        Self::RootCause,
        Self::Impact,
        Self::Remediation,
        Self::Commands,
        Self::FinalReview,
        Self::Notes,
        Self::Log,
    ];

    /// Whether this section is opt-in: not created by `scaffold`, and its
    /// tab appears only once the file exists. Core sections return `false`.
    #[must_use]
    pub fn is_optional(self) -> bool {
        matches!(self, Self::Commands)
    }

    /// The file name backing this section inside the workspace directory.
    #[must_use]
    pub fn file_name(self) -> &'static str {
        match self {
            Self::Summary => "summary.md",
            Self::Timeline => "timeline.md",
            Self::RootCause => "root-cause.md",
            Self::Impact => "impact.md",
            Self::Remediation => "remediation.md",
            Self::Commands => "commands.md",
            Self::FinalReview => "final-review.md",
            Self::Notes => "notes.md",
            Self::Log => "log.md",
        }
    }

    /// The human-readable tab title.
    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Self::Summary => "Summary",
            Self::Timeline => "Timeline",
            Self::RootCause => "Root Cause",
            Self::Impact => "Impact",
            Self::Remediation => "Fix",
            Self::Commands => "Commands",
            Self::FinalReview => "Final Review",
            Self::Notes => "Notes",
            Self::Log => "Log",
        }
    }
}
