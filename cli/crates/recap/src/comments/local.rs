//! Local per-recap comment state: the fetched threads, what you have read,
//! and agents' reviews of threads.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{Thread, Threads};
use crate::error::{Error, Result};
use crate::store::Store;

/// `recaps/<id>/comments/`.
#[must_use]
pub fn dir(recap_dir: &Path) -> PathBuf {
    recap_dir.join("comments")
}

/// Where the last sync failure is written for the TUI to show.
#[must_use]
pub fn error_path(store: &Store) -> PathBuf {
    store.recaps_dir().join(".comments-error.txt")
}

fn read_json<T: for<'de> Deserialize<'de> + Default>(path: &Path) -> T {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

pub(super) fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
    }
    beagle_utils::fs::write_atomic(path, serde_json::to_string_pretty(value)?.as_bytes())
        .map_err(|e| Error::io(path, e))
}

/// The threads last fetched for a recap (empty when never synced).
#[must_use]
pub fn load(recap_dir: &Path) -> Threads {
    read_json(&dir(recap_dir).join("threads.json"))
}

/// Thread id → the id of the last comment you have seen in it.
pub type ReadMarks = BTreeMap<String, String>;

/// What you have read.
#[must_use]
pub fn load_read(recap_dir: &Path) -> ReadMarks {
    read_json(&dir(recap_dir).join("read.json"))
}

/// Saves what you have read.
///
/// # Errors
/// [`Error::Io`] when it can't be written.
pub fn save_read(recap_dir: &Path, marks: &ReadMarks) -> Result<()> {
    write_json(&dir(recap_dir).join("read.json"), marks)
}

/// Whether you have seen the thread's newest comment.
#[must_use]
pub fn is_read(thread: &Thread, marks: &ReadMarks) -> bool {
    match (thread.last_counted(), marks.get(&thread.id)) {
        (Some(last), Some(seen)) => &last.id == seen,
        (None, _) => true,
        (Some(_), None) => false,
    }
}

/// An agent's review of a thread, and the comment id it answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Review {
    /// The comment it was written for.
    pub covers: String,
    /// Markdown, the covers line stripped.
    pub body: String,
}

/// The heading of a review's short summary — the TUI's right-hand pane.
pub const IN_SHORT: &str = "## In short";

impl Review {
    /// The review split into its `## In short` section (for the summary
    /// pane) and the rest (shown under the thread). No such section: no
    /// short summary, the whole body is the rest.
    #[must_use]
    pub fn split(&self) -> (Option<String>, String) {
        let mut short = Vec::new();
        let mut rest = Vec::new();
        let mut in_short = false;
        for line in self.body.lines() {
            if line.trim() == IN_SHORT {
                in_short = true;
                continue;
            }
            if in_short && line.starts_with("## ") {
                in_short = false;
            }
            if in_short {
                short.push(line);
            } else {
                rest.push(line);
            }
        }
        let short = short.join("\n").trim().to_owned();
        (
            (!short.is_empty()).then_some(short),
            rest.join("\n").trim().to_owned(),
        )
    }
}

const COVERS: &str = "<!-- covers: ";

/// The agent's review of `thread_id`, if there is one.
#[must_use]
pub fn load_review(recap_dir: &Path, thread_id: &str) -> Option<Review> {
    let text = std::fs::read_to_string(
        dir(recap_dir)
            .join("reviews")
            .join(format!("{thread_id}.md")),
    )
    .ok()?;
    let (first, rest) = text.split_once('\n').unwrap_or((&text, ""));
    let covers = first
        .strip_prefix(COVERS)?
        .trim_end_matches("-->")
        .trim()
        .to_owned();
    Some(Review {
        covers,
        body: rest.trim().to_owned(),
    })
}

/// Whether the agent's review still answers the thread: it covers the
/// newest comment from someone other than the PR's author.
#[must_use]
pub fn is_reviewed(recap_dir: &Path, thread: &Thread, pr_author: &str) -> bool {
    match (
        thread.last_from_others(pr_author),
        load_review(recap_dir, &thread.id),
    ) {
        (None, _) => true, // nothing to answer: only the PR author in it
        (Some(last), Some(review)) => review.covers == last.id,
        (Some(_), None) => false,
    }
}

/// Installs an agent's review of a thread, stamped with the comment it
/// answers.
///
/// # Errors
/// [`Error::NoSuchRecap`]-style text when the thread is unknown; [`Error::Io`].
pub fn save_review(
    recap_dir: &Path,
    thread_id: &str,
    pr_author: &str,
    body: &str,
) -> Result<PathBuf> {
    let threads = load(recap_dir);
    let thread = threads
        .threads
        .iter()
        .find(|t| t.id == thread_id)
        .ok_or_else(|| Error::Tool {
            tool: "recap",
            message: format!("no thread {thread_id} — run `beagle recap comments sync` first"),
        })?;
    let covers = thread
        .last_from_others(pr_author)
        .or_else(|| thread.last_counted())
        .map(|c| c.id.clone())
        .unwrap_or_default();
    let path = dir(recap_dir)
        .join("reviews")
        .join(format!("{thread_id}.md"));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
    }
    std::fs::write(&path, format!("{COVERS}{covers} -->\n{}\n", body.trim()))
        .map_err(|e| Error::io(&path, e))?;
    Ok(path)
}

/// Deletes the local review and read mark of every thread no longer in
/// `threads` (resolved, deleted, or now only the author's), so nothing
/// for a gone thread lingers. Best effort: all of it is re-creatable.
pub(super) fn forget_gone(recap_dir: &Path, threads: &[Thread]) {
    let live = |id: &str| threads.iter().any(|t| t.id == id);
    if let Ok(entries) = std::fs::read_dir(dir(recap_dir).join("reviews")) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if let Some(id) = name.strip_suffix(".md") {
                if !live(id) {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
    }
    let mut read = load_read(recap_dir);
    let before = read.len();
    read.retain(|id, _| live(id));
    if read.len() != before {
        let _ = save_read(recap_dir, &read);
    }
}
