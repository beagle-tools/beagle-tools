//! Following the event spools (`$XDG_STATE_HOME/beagle/events/<tool>.jsonl`)
//! from where serve last stopped, with the cursor kept on disk.

use std::collections::BTreeMap;
use std::io::{Read as _, Seek as _, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::Duration;

use beagle_scout::events::{spool, Record};
use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

/// Events older than this when serve reads them are dropped, so a Mac that
/// slept doesn't wake the phone with stale news.
pub const MAX_AGE: Duration = Duration::from_mins(10);

/// Where serve got to in one spool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cursor {
    /// The file's identity (inode), so a rotation is noticed.
    pub file: u64,
    /// Bytes already read.
    pub offset: u64,
}

/// Reads new events from every tool's spool.
#[derive(Debug)]
pub struct Follower {
    dir: PathBuf,
    cursor_path: PathBuf,
    tools: Vec<String>,
    cursors: BTreeMap<String, Cursor>,
}

impl Follower {
    /// Follows `tools`' spools in `dir`, keeping the cursors in
    /// `cursor_path` (a missing or broken one starts at each file's end).
    #[must_use]
    pub fn new(dir: PathBuf, cursor_path: PathBuf, tools: Vec<String>) -> Self {
        let cursors = beagle_utils::fs::read_optional(&cursor_path)
            .ok()
            .flatten()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        Self {
            dir,
            cursor_path,
            tools,
            cursors,
        }
    }

    /// The events written since the last poll, at most [`MAX_AGE`] old at
    /// `now`, oldest first. Lines that don't parse are skipped and
    /// reported in the second list.
    pub fn poll(&mut self, now: OffsetDateTime) -> (Vec<Record>, Vec<String>) {
        let mut records = Vec::new();
        let mut problems = Vec::new();
        let mut moved = false;
        for tool in self.tools.clone() {
            let path = spool(&self.dir, &tool);
            let Some(identity) = identity_of(&path) else {
                // No spool yet: when one appears, read it from the start.
                if let std::collections::btree_map::Entry::Vacant(e) = self.cursors.entry(tool) {
                    e.insert(Cursor { file: 0, offset: 0 });
                    moved = true;
                }
                continue;
            };
            let len = std::fs::metadata(&path).map_or(0, |m| m.len());
            let Some(cursor) = self.cursors.get(&tool).copied() else {
                // First time: start at the end, never replay history.
                self.cursors.insert(
                    tool,
                    Cursor {
                        file: identity,
                        offset: len,
                    },
                );
                moved = true;
                continue;
            };
            let mut lines = Vec::new();
            let start = if cursor.file == identity {
                if cursor.offset > len {
                    0
                } else {
                    cursor.offset
                }
            } else {
                // Rotated: finish the old file if it is the one we were in.
                let old = path.with_extension("jsonl.1");
                if identity_of(&old) == Some(cursor.file) {
                    read_lines(&old, cursor.offset, &mut lines);
                }
                0
            };
            let end = read_lines(&path, start, &mut lines);
            if end != cursor.offset || identity != cursor.file {
                self.cursors.insert(
                    tool,
                    Cursor {
                        file: identity,
                        offset: end,
                    },
                );
                moved = true;
            }
            for line in lines {
                match serde_json::from_str::<Record>(&line) {
                    Ok(r) if fresh(&r, now) => records.push(r),
                    Ok(_) => {}
                    Err(e) => problems.push(format!("{}: unreadable event: {e}", path.display())),
                }
            }
        }
        if moved {
            if let Ok(text) = serde_json::to_string(&self.cursors) {
                let _ = beagle_utils::fs::write_atomic(&self.cursor_path, text.as_bytes());
            }
        }
        (records, problems)
    }
}

/// Whether `r` happened at most [`MAX_AGE`] before `now` (an unreadable
/// time counts as stale).
fn fresh(r: &Record, now: OffsetDateTime) -> bool {
    OffsetDateTime::parse(&r.time, &Rfc3339).is_ok_and(|t| now - t <= MAX_AGE)
}

/// Reads the whole lines of `path` from byte `start` into `out`; returns
/// the offset after the last whole line (a half-written line waits for
/// the next poll).
fn read_lines(path: &Path, start: u64, out: &mut Vec<String>) -> u64 {
    let Ok(mut file) = std::fs::File::open(path) else {
        return start;
    };
    let mut bytes = Vec::new();
    if file.seek(SeekFrom::Start(start)).is_err() || file.read_to_end(&mut bytes).is_err() {
        return start;
    }
    let Some(last) = bytes.iter().rposition(|b| *b == b'\n') else {
        return start;
    };
    let whole = &bytes[..=last];
    out.extend(
        String::from_utf8_lossy(whole)
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(str::to_owned),
    );
    start + u64::try_from(whole.len()).unwrap_or(u64::MAX)
}

/// A file's identity: its inode on Unix (elsewhere a constant, so rotation
/// shows only as a shorter file).
fn identity_of(path: &Path) -> Option<u64> {
    let meta = std::fs::metadata(path).ok()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        Some(meta.ino())
    }
    #[cfg(not(unix))]
    {
        let _ = meta;
        Some(0)
    }
}
