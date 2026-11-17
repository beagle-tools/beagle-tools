//! Attached Claude Code sessions: parse Claude's transcript into scout's own
//! compact format, and keep a copy in the store.
//!
//! Claude Code writes every session to
//! `~/.claude/projects/<dir>/<session-id>.jsonl`, one JSON object per line:
//!
//! ```text
//! type        what                                             scout entry
//! ─────────── ──────────────────────────────────────────────── ───────────
//! user        content is a string, origin.kind = "human":       Prompt
//!             your prompt (slash commands as <command-name>)
//! assistant   content blocks: thinking (text often empty,       Thinking
//!             only a signature + thinkingDurationMs), text,     Text
//!             tool_use {id, name, input}                        Tool
//! user        content blocks: tool_result {tool_use_id,         Result
//!             content, is_error}, plus toolUseResult details
//! user        origin.kind = "task-notification"                 Notice
//! others      isMeta (injected skill text), system, attachment, (skipped)
//!             api-request, file-history, mode, …
//! ```
//!
//! [`sync`] converts new transcript lines into [`Entry`] lines appended to
//! `<project>/sessions/<session-id>.jsonl`, remembering how far it got in a
//! `.cursor` file, so it is cheap to call often. The copy outlives Claude's
//! own transcript cleanup and is independent of its internal format.
//! Everything here is best effort: an unrecognised line is skipped, never an
//! error.

use std::fs::{self, File, OpenOptions};
use std::io::{Read as _, Seek as _, SeekFrom, Write as _};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Error, Result};
use crate::store::{Project, Task};

/// The per-project directory holding converted sessions.
pub const CACHE_DIR: &str = "sessions";

/// Longest text kept for a prompt, reply or thought.
const MAX_TEXT: usize = 16 * 1024;
/// Longest tool output kept.
const MAX_RESULT: usize = 4 * 1024;
/// Longest command or summary kept.
const MAX_SUMMARY: usize = 2 * 1024;

/// One thing that happened in a session, in scout's format.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Entry {
    /// The user's prompt (or slash command).
    Prompt {
        /// RFC 3339 time, as Claude recorded it.
        ts: String,
        /// The prompt text.
        text: String,
        /// The session's working directory (paths are shown relative to it).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cwd: Option<String>,
    },
    /// Claude's thinking: its text when Claude Code stored it, else only
    /// how long it took.
    Thinking {
        /// RFC 3339 time.
        ts: String,
        /// The thought, when stored.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text: Option<String>,
        /// How long it thought.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ms: Option<u64>,
    },
    /// Claude's reply text (Markdown).
    Text {
        /// RFC 3339 time.
        ts: String,
        /// The reply.
        text: String,
    },
    /// A tool call.
    Tool {
        /// RFC 3339 time.
        ts: String,
        /// The tool-use id, pairing it with its result.
        id: String,
        /// The tool (`Bash`, `Edit`, `Read`, …).
        name: String,
        /// The gist: the command, the file, the pattern, the question.
        summary: String,
        /// Extra context: a Bash description, an agent type.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },
    /// A tool's result.
    Result {
        /// RFC 3339 time.
        ts: String,
        /// The tool-use id it answers.
        id: String,
        /// The output (capped).
        text: String,
        /// Whether the tool failed.
        #[serde(default, skip_serializing_if = "is_false")]
        error: bool,
    },
    /// A file the session created or changed (from Edit/Write results).
    Change {
        /// RFC 3339 time.
        ts: String,
        /// The tool-use id that made it.
        id: String,
        /// The file.
        path: String,
        /// True when the file was new.
        #[serde(default, skip_serializing_if = "is_false")]
        created: bool,
        /// Lines added.
        added: usize,
        /// Lines removed.
        removed: usize,
    },
    /// Something the harness reported (a background task finished, …).
    Notice {
        /// RFC 3339 time.
        ts: String,
        /// What happened.
        text: String,
    },
}

#[allow(clippy::trivially_copy_pass_by_ref)] // serde's skip_serializing_if passes a reference
fn is_false(b: &bool) -> bool {
    !*b
}

impl Entry {
    /// When it happened (RFC 3339).
    #[must_use]
    pub fn ts(&self) -> &str {
        match self {
            Self::Prompt { ts, .. }
            | Self::Thinking { ts, .. }
            | Self::Text { ts, .. }
            | Self::Tool { ts, .. }
            | Self::Result { ts, .. }
            | Self::Change { ts, .. }
            | Self::Notice { ts, .. } => ts,
        }
    }
}

mod parse;
mod sync;

pub use parse::parse_line;
pub use sync::{
    cache_file, etl, find_transcript, is_session_id, read_cache, sync, task_slice, view_file,
};

#[cfg(test)]
mod tests;
