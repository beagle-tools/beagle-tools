//! Structured diff types. The Rust side parses raw unified diffs (see
//! [`crate::diff_parse`]) so the frontend only ever renders structure.

use serde::{Deserialize, Serialize};

/// What a [`DiffLine`] represents relative to the base.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineKind {
    /// Unchanged line, present on both sides.
    Context,
    /// Line only on the new side.
    Added,
    /// Line only on the old side.
    Removed,
}

/// One rendered line of a diff hunk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffLine {
    /// Added, removed, or context.
    pub kind: LineKind,
    /// Line number in the old file (None for added lines).
    pub old_line: Option<u64>,
    /// Line number in the new file (None for removed lines).
    pub new_line: Option<u64>,
    /// Line text without the leading `+`/`-`/space marker or trailing newline.
    pub content: String,
}

/// One `@@` hunk of a file diff.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hunk {
    /// First line of the hunk in the old file (1-based; 0 for an empty side).
    pub old_start: u64,
    /// Number of old-file lines the hunk spans.
    pub old_count: u64,
    /// First line of the hunk in the new file (1-based; 0 for an empty side).
    pub new_start: u64,
    /// Number of new-file lines the hunk spans.
    pub new_count: u64,
    /// Trailing text on the `@@` header (usually the enclosing function).
    pub section: String,
    /// Lines in diff order.
    pub lines: Vec<DiffLine>,
}

/// How a file changed in the PR.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileStatus {
    /// New file.
    Added,
    /// Deleted file.
    Removed,
    /// Edited in place.
    Modified,
    /// Moved (possibly also edited); see [`FileDiff::old_path`].
    Renamed,
}

/// The parsed diff of one file in a PR.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDiff {
    /// Path before the change (same as `new_path` unless renamed).
    pub old_path: String,
    /// Path after the change (same as `old_path` for deletions).
    pub new_path: String,
    /// Added / removed / modified / renamed.
    pub status: FileStatus,
    /// Parsed hunks; empty for binary files and pure renames.
    pub hunks: Vec<Hunk>,
    /// Lines added in this file.
    pub additions: u64,
    /// Lines removed in this file.
    pub deletions: u64,
    /// True when the diff body was binary or elided.
    pub is_binary: bool,
}

impl FileDiff {
    /// The path the UI should display (new path, or old path on delete).
    pub fn display_path(&self) -> &str {
        if self.status == FileStatus::Removed {
            &self.old_path
        } else {
            &self.new_path
        }
    }
}
