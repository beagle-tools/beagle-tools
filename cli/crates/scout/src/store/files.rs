//! File helpers: capped reads, task loading, atomic writes and the log line
//! format.

use super::*;

/// A [`SystemTime`] as the store's timestamp type.
pub(super) fn now_from(t: SystemTime) -> OffsetDateTime {
    OffsetDateTime::from(t)
        .replace_nanosecond(0)
        .unwrap_or_else(|_| now())
}

/// The latest modification time of `path`, if any.
#[must_use]
pub fn modified(path: &Path) -> Option<SystemTime> {
    mtime(path)
}

/// Reads a file, refusing anything over 4 MB.
///
/// # Errors
/// [`Error::TooLarge`] or [`Error::Io`].
pub fn read_capped(path: &Path) -> Result<String> {
    let len = fs::metadata(path).map_err(Error::io(path))?.len();
    if len > MAX_FILE_BYTES {
        return Err(Error::TooLarge {
            path: path.to_owned(),
        });
    }
    fs::read_to_string(path).map_err(Error::io(path))
}

/// Splits a task file stem `NN-slug` into its number and slug.
#[must_use]
pub fn split_task_stem(stem: &str) -> Option<(u32, &str)> {
    let (number, slug) = stem.split_once('-')?;
    if number.is_empty() || !number.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Slug::parse(slug).ok()?;
    Some((number.parse().ok()?, slug))
}

pub(super) fn load_dir(id: Slug, dir: PathBuf) -> Result<(Project, Vec<String>)> {
    let file = dir.join(PROJECT_FILE);
    let source = read_capped(&file)?;
    let (meta, _) = frontmatter::parse::<ProjectMeta>(&file, &source)?;
    let mut last_activity = mtime(&file).max(mtime(&dir.join(LOG_FILE)));
    let mut tasks = Vec::new();
    let mut warnings = Vec::new();
    if let Ok(entries) = fs::read_dir(dir.join(TASKS_DIR)) {
        for path in entries.flatten().map(|e| e.path()) {
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            match load_task(&path) {
                Ok(Some(task)) => {
                    last_activity = last_activity.max(task.modified);
                    tasks.push(task);
                }
                Ok(None) => {}
                Err(e) => warnings.push(e.to_string()),
            }
        }
    }
    tasks.sort_by(|a, b| a.number.cmp(&b.number).then_with(|| a.id.cmp(&b.id)));
    let project = Project {
        id,
        meta,
        dir,
        tasks,
        last_activity,
    };
    Ok((project, warnings))
}

/// Loads a task file; `Ok(None)` when its name is not `NN-slug.md`.
pub(super) fn load_task(path: &Path) -> Result<Option<Task>> {
    let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
        return Ok(None);
    };
    let Some((number, slug)) = split_task_stem(stem) else {
        return Ok(None);
    };
    let source = read_capped(path)?;
    let (meta, body) = frontmatter::parse::<TaskMeta>(path, &source)?;
    Ok(Some(Task {
        id: stem.to_owned(),
        number,
        slug: slug.to_owned(),
        meta,
        path: path.to_owned(),
        modified: mtime(path),
        checklist: markdown::checklist_stats(&body),
    }))
}

pub(super) fn update_doc<T: DeserializeOwned + Serialize>(
    path: &Path,
    edit: impl FnOnce(&mut T),
) -> Result<()> {
    let source = read_capped(path)?;
    let (mut meta, body) = frontmatter::parse::<T>(path, &source)?;
    edit(&mut meta);
    write_atomic(path, &frontmatter::render(path, &meta, &body)?)
}

/// Writes via a sibling temp file and a rename, so a reader (the TUI) never
/// sees a half-written file.
pub(super) fn write_atomic(path: &Path, contents: &str) -> Result<()> {
    beagle_utils::fs::write_atomic(path, contents.as_bytes()).map_err(Error::io(path))
}

pub(super) fn append_line(path: &Path, line: &str) -> Result<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(Error::io(path))?;
    writeln!(file, "{line}").map_err(Error::io(path))
}

pub(super) fn log_line(at: OffsetDateTime, task: Option<&str>, message: &str) -> String {
    let stamp = at
        .format(format_description!(
            "[year]-[month]-[day] [hour]:[minute] UTC"
        ))
        .unwrap_or_default();
    let message = message.split_whitespace().collect::<Vec<_>>().join(" ");
    match task {
        Some(task) => format!("- **{stamp}** `{task}` {message}"),
        None => format!("- **{stamp}** {message}"),
    }
}

pub(super) fn now() -> OffsetDateTime {
    let now = OffsetDateTime::now_utc();
    now.replace_nanosecond(0).unwrap_or(now)
}

pub(super) fn mtime(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// The first paragraph under `## <heading>`, joined into one line; `None`
/// when the section is missing, empty, or still the template placeholder.
pub(super) fn section_first_paragraph(body: &str, heading: &str) -> Option<String> {
    let mut lines = body
        .lines()
        .skip_while(|l| l.trim() != format!("## {heading}"));
    lines.next()?;
    let paragraph: Vec<&str> = lines
        .take_while(|l| !l.starts_with("## "))
        .map(str::trim)
        .skip_while(|l| l.is_empty())
        .take_while(|l| !l.is_empty())
        .collect();
    let text = paragraph.join(" ");
    (!text.is_empty() && !text.starts_with('<')).then_some(text)
}
