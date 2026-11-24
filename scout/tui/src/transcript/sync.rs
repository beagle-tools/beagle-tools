//! Locating transcripts and syncing them, incrementally, into a project's
//! `sessions/` cache and per-task rendered views.

use super::*;

/// Session ids become file names, so only `[A-Za-z0-9_-]` is accepted.
#[must_use]
pub fn is_session_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Claude Code's transcript for `session`: `~/.claude/projects/*/<id>.jsonl`.
#[must_use]
pub fn find_transcript(home: &Path, session: &str) -> Option<PathBuf> {
    if !is_session_id(session) {
        return None;
    }
    let projects = home.join(".claude").join("projects");
    fs::read_dir(projects)
        .ok()?
        .flatten()
        .map(|dir| dir.path().join(format!("{session}.jsonl")))
        .find(|path| path.is_file())
}

/// The converted copy of `session` for the project in `project_dir`.
#[must_use]
pub fn cache_file(project_dir: &Path, session: &str) -> PathBuf {
    project_dir.join(CACHE_DIR).join(format!("{session}.jsonl"))
}

pub(super) fn cursor_file(cache: &Path) -> PathBuf {
    cache.with_extension("cursor")
}

/// The rendered view of a session for one task, next to the session's
/// entries: `<session>.<task-id>.log`. This is what the TUI shows.
#[must_use]
pub fn view_file(cache: &Path, task_id: &str) -> PathBuf {
    let session = cache
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    cache.with_file_name(format!("{session}.{task_id}.log"))
}

/// The part of a session that belongs to `task`: from the prompt before
/// the agent first started (or attached) it, to the agent's last mention of
/// it (a status change, a `beagle scout log --task`) and that call's output; to
/// the end while the task is still being worked. Work on other tasks in
/// between is kept, so pauses and resumes read naturally. The whole session
/// when there is no marker at all (attached by hand).
#[must_use]
pub fn task_slice<'a>(entries: &'a [Entry], project: &Project, task: &Task) -> &'a [Entry] {
    use crate::watch::{scout_calls, ScoutCall};
    let is_this = |p: &str, t: &str| {
        p == project.id.as_str() && project.resolve_task(t).is_some_and(|r| r.id == task.id)
    };
    // (starts the task, mentions the task) for each entry.
    let mentions = |entry: &Entry| -> (bool, bool) {
        let Entry::Tool { name, summary, .. } = entry else {
            return (false, false);
        };
        if name != "Bash" {
            return (false, false);
        }
        let mut starts = false;
        let mut mentions = false;
        for call in scout_calls(summary) {
            match call {
                ScoutCall::Task {
                    project,
                    task,
                    status,
                } if is_this(&project, &task) => {
                    mentions = true;
                    starts |= status == crate::model::TaskStatus::Working;
                }
                ScoutCall::Attach { project, task } if is_this(&project, &task) => {
                    mentions = true;
                    starts = true;
                }
                ScoutCall::Log {
                    project,
                    task: Some(task),
                    ..
                } if is_this(&project, &task) => mentions = true,
                _ => {}
            }
        }
        (starts, mentions)
    };
    let Some(start) = entries.iter().position(|e| mentions(e).0) else {
        return entries;
    };
    let from = entries[..start]
        .iter()
        .rposition(|e| matches!(e, Entry::Prompt { .. }))
        .unwrap_or(start);
    if task.meta.status == crate::model::TaskStatus::Working {
        return &entries[from..];
    }
    let last = entries.iter().rposition(|e| mentions(e).1).unwrap_or(start);
    // Keep that call's output and Claude's reply to it.
    let end = entries[last + 1..]
        .iter()
        .position(|e| matches!(e, Entry::Tool { .. } | Entry::Prompt { .. }))
        .map_or(entries.len(), |i| last + 1 + i);
    &entries[from..end]
}

/// Bumped whenever [`Entry`], the parsing, or the slicing changes, so
/// existing copies (and their views) are rebuilt from the transcript.
const FORMAT_VERSION: u32 = 4;

/// Appends whatever the transcript gained since the last sync to `cache`,
/// in scout's format. Returns how many entries were added (0 = nothing
/// new; nothing is written then, so watchers stay quiet). A transcript that
/// shrank (replaced) is converted again from the start.
///
/// # Errors
/// [`Error::Io`] if the transcript or cache cannot be read or written.
pub fn sync(transcript: &Path, cache: &Path) -> Result<usize> {
    let len = fs::metadata(transcript)
        .map_err(Error::io(transcript))?
        .len();
    let cursor = cursor_file(cache);
    // The cursor is `<format version> <byte offset>`; anything else (an
    // older format) means "start over".
    let saved = fs::read_to_string(&cursor).ok().and_then(|s| {
        let (version, offset) = s.trim().split_once(' ')?;
        (version.parse::<u32>().ok()? == FORMAT_VERSION).then(|| offset.parse::<u64>().ok())?
    });
    let mut offset = saved.unwrap_or(0);
    if saved.is_none() || len < offset {
        offset = 0;
        let _ = fs::remove_file(cache); // start over; missing is fine
    }
    if len == offset {
        return Ok(0);
    }
    let mut file = File::open(transcript).map_err(Error::io(transcript))?;
    file.seek(SeekFrom::Start(offset))
        .map_err(Error::io(transcript))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(Error::io(transcript))?;
    // Only complete lines: the last one may still be being written.
    let Some(last_newline) = bytes.iter().rposition(|b| *b == b'\n') else {
        return Ok(0);
    };
    let complete = &bytes[..=last_newline];
    let mut out = String::new();
    let mut added = 0;
    for line in String::from_utf8_lossy(complete).lines() {
        for entry in parse_line(line) {
            out.push_str(&serde_json::to_string(&entry)?);
            out.push('\n');
            added += 1;
        }
    }
    if let Some(dir) = cache.parent() {
        fs::create_dir_all(dir).map_err(Error::io(dir))?;
    }
    if added > 0 {
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(cache)
            .map_err(Error::io(cache))?;
        f.write_all(out.as_bytes()).map_err(Error::io(cache))?;
    }
    let next = offset + complete.len() as u64;
    fs::write(&cursor, format!("{FORMAT_VERSION} {next}")).map_err(Error::io(&cursor))?;
    Ok(added)
}

/// The whole pipeline for one session in one project: **extract** the new
/// transcript lines, **transform** them into entries (appended to the
/// `.jsonl`) and into the rendered view, and **load** that view into the
/// `.log` file the TUI displays. Cheap when nothing changed. A lock keeps
/// two runners (a hook and the TUI) from appending the same lines twice;
/// if another run holds it, this one skips.
///
/// Returns whether the view was (re)written.
///
/// # Errors
/// [`Error::Io`] if the transcript, cache, or view cannot be read or written.
pub fn etl(
    transcript: &Path,
    project_dir: &Path,
    session: &str,
    store: &crate::store::Store,
) -> Result<bool> {
    let cache = cache_file(project_dir, session);
    if let Some(dir) = cache.parent() {
        fs::create_dir_all(dir).map_err(Error::io(dir))?;
    }
    let Some(_lock) = Lock::take(&cache.with_extension("lock")) else {
        return Ok(false);
    };
    let added = sync(transcript, &cache)?;
    let project_id = project_dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let Ok(project) = store.project(&project_id) else {
        return Ok(false);
    };
    let tasks: Vec<&Task> = project
        .tasks
        .iter()
        .filter(|t| t.meta.sessions.iter().any(|s| s == session))
        .collect();
    let all_rendered = tasks.iter().all(|t| view_file(&cache, &t.id).exists());
    if added == 0 && all_rendered {
        return Ok(false);
    }
    let entries = read_cache(&cache);
    for task in tasks {
        let slice = task_slice(&entries, &project, task);
        let text = crate::transcript_view::render(&[(session.to_owned(), slice.to_vec())], store);
        let view = view_file(&cache, &task.id);
        let ansi = crate::transcript_view::to_ansi(&text);
        beagle_utils::fs::write_atomic(&view, ansi.as_bytes()).map_err(Error::io(&view))?;
    }
    Ok(true)
}

/// A lock file held for the life of the value. Stale locks (a crashed run)
/// are broken after a minute.
pub(super) struct Lock(PathBuf);

impl Lock {
    pub(super) fn take(path: &Path) -> Option<Self> {
        let stale = fs::metadata(path)
            .and_then(|m| m.modified())
            .is_ok_and(|t| t.elapsed().unwrap_or_default() > std::time::Duration::from_secs(60));
        if stale {
            let _ = fs::remove_file(path);
        }
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .ok()
            .map(|_| Self(path.to_owned()))
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

/// Reads a converted session.
#[must_use]
pub fn read_cache(cache: &Path) -> Vec<Entry> {
    fs::read_to_string(cache)
        .map(|text| {
            text.lines()
                .filter_map(|l| serde_json::from_str(l).ok())
                .collect()
        })
        .unwrap_or_default()
}
