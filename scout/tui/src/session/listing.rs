//! Listing recorded sessions, describing them, and picking one.

use super::{dir, file, fs, parse_line, Event, Path, PathBuf, SystemTime, Value};

/// What `--list` and session picking need to know about a recording.
#[derive(Debug, Clone)]
pub struct SessionInfo {
    /// The Claude Code session id.
    pub id: String,
    /// The recording file.
    pub path: PathBuf,
    /// When the recording last changed.
    pub modified: SystemTime,
    /// The Herdr tab of the most recent command.
    pub tab: Option<String>,
    /// The working directory of the most recent command.
    pub cwd: Option<String>,
    /// The most recent command line.
    pub last_command: Option<String>,
    /// Claude Code's transcript file, if the hook reported it.
    pub transcript: Option<PathBuf>,
    /// How many commands were recorded.
    pub commands: usize,
}

/// Every recording, newest first.
#[must_use]
pub fn list(root: &Path) -> Vec<SessionInfo> {
    let Ok(entries) = fs::read_dir(dir(root)) else {
        return Vec::new();
    };
    let mut sessions: Vec<SessionInfo> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "jsonl"))
        .filter_map(|path| info(&path))
        .collect();
    sessions.sort_by(|a, b| b.modified.cmp(&a.modified).then_with(|| a.id.cmp(&b.id)));
    sessions
}

pub(super) fn info(path: &Path) -> Option<SessionInfo> {
    let id = path.file_stem()?.to_str()?.to_owned();
    let modified = fs::metadata(path).and_then(|m| m.modified()).ok()?;
    let source = fs::read_to_string(path).ok()?;
    let mut summary = SessionInfo {
        id,
        path: path.to_owned(),
        modified,
        tab: None,
        cwd: None,
        last_command: None,
        transcript: None,
        commands: 0,
    };
    for event in source.lines().filter_map(parse_line) {
        if let Event::Start {
            command,
            cwd,
            tab,
            transcript,
            ..
        } = event
        {
            summary.commands += 1;
            summary.transcript = transcript.map(PathBuf::from).or(summary.transcript);
            summary.last_command = Some(command);
            summary.cwd = cwd.or(summary.cwd);
            summary.tab = tab.or(summary.tab);
        }
    }
    Some(summary)
}

/// What `claude --resume` shows for a session: its AI-generated title and
/// your most recent prompt, read from the tail of Claude Code's transcript.
/// Best effort: these entries are Claude Code internals, so anything missing
/// or unreadable is simply `None`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Description {
    /// The session title Claude Code generated (`ai-title`).
    pub title: Option<String>,
    /// Your most recent prompt (`last-prompt`).
    pub last_prompt: Option<String>,
}

/// How much of the transcript's end is searched for the title and prompt.
pub(super) const DESCRIBE_TAIL_BYTES: u64 = 512 * 1024;

/// Reads [`Description`] for a recorded session.
#[must_use]
pub fn describe(info: &SessionInfo) -> Description {
    info.transcript
        .as_deref()
        .and_then(|path| read_tail(path, DESCRIBE_TAIL_BYTES))
        .map(|tail| describe_transcript(&tail))
        .unwrap_or_default()
}

pub(super) fn read_tail(path: &Path, max: u64) -> Option<String> {
    use std::io::{Read as _, Seek as _, SeekFrom};
    let mut file = fs::File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    file.seek(SeekFrom::Start(len.saturating_sub(max))).ok()?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// The last `ai-title` and `last-prompt` entries in transcript lines.
pub(super) fn describe_transcript(text: &str) -> Description {
    let mut found = Description::default();
    for line in text.lines().rev() {
        if found.title.is_some() && found.last_prompt.is_some() {
            break;
        }
        // Cheap substring checks first: transcripts are large and most
        // lines are neither.
        let wanted = (found.title.is_none() && line.contains("\"ai-title\""))
            || (found.last_prompt.is_none() && line.contains("\"last-prompt\""));
        if !wanted {
            continue;
        }
        let Ok(entry) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let field = |key: &str| {
            entry
                .get(key)
                .and_then(Value::as_str)
                .map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "))
                .filter(|s| !s.is_empty())
        };
        match entry.get("type").and_then(Value::as_str) {
            Some("ai-title") if found.title.is_none() => found.title = field("aiTitle"),
            Some("last-prompt") if found.last_prompt.is_none() => {
                found.last_prompt = field("lastPrompt");
            }
            _ => {}
        }
    }
    found
}

/// Expands `wanted` to a full session id: itself if recorded, else the one
/// recorded session it is a prefix of. Unknown ids come back unchanged (the
/// session may not have run a command yet). `Err` lists the ids an
/// ambiguous prefix matches.
///
/// # Errors
/// The matching ids, when `wanted` is a prefix of more than one.
pub fn resolve(root: &Path, wanted: &str) -> std::result::Result<String, Vec<String>> {
    if file(root, wanted).exists() {
        return Ok(wanted.to_owned());
    }
    let mut matches: Vec<String> = list(root)
        .into_iter()
        .map(|s| s.id)
        .filter(|id| id.starts_with(wanted))
        .collect();
    match matches.len() {
        0 => Ok(wanted.to_owned()),
        1 => Ok(matches.remove(0)),
        _ => Err(matches),
    }
}

/// Picks the session a watcher most likely wants: the newest one from the
/// same Herdr tab, else the newest from the same directory, else the newest.
/// `sessions` must be newest first (as [`list`] returns them).
#[must_use]
pub fn pick<'a>(
    sessions: &'a [SessionInfo],
    tab: Option<&str>,
    cwd: &Path,
) -> Option<&'a SessionInfo> {
    let cwd = cwd.to_string_lossy();
    tab.and_then(|tab| sessions.iter().find(|s| s.tab.as_deref() == Some(tab)))
        .or_else(|| sessions.iter().find(|s| s.cwd.as_deref() == Some(&*cwd)))
        .or_else(|| sessions.first())
}
