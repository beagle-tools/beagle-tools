//! Project-local `.beagle` files: discovery (git-style, walking up),
//! layering over the global config, and creation by `beagle init`.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

use super::{load, load_default, Config, CONFIG_ENV};

/// Project config file name, discovered git-style: the nearest `.beagle`
/// at or above the working directory pins the config for everything run
/// under it, the way `.git` pins a repository.
pub const PROJECT_FILE: &str = ".beagle";

/// The nearest `.beagle` file at or above `start`, if any.
#[must_use]
pub fn find_project_file(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .map(|dir| dir.join(PROJECT_FILE))
        .find(|candidate| candidate.is_file())
}

/// Loads the effective config for a command run in `start`: the nearest
/// `.beagle` walking up from there when one exists, else the global config
/// alone. Fields the `.beagle` sets win; anything it leaves unset falls
/// back to the global config, so a project file that only pins `root`
/// keeps the global editor and notification settings.
///
/// The root is special — a `.beagle` always determines it: a relative
/// `root` resolves against the `.beagle` file's directory, and no `root`
/// at all pins the root to that directory itself, so an **empty**
/// `.beagle` marks a store root exactly like `.git` marks a repo.
///
/// `$BEAGLE_CONFIG` skips project discovery entirely — an explicit
/// override should never lose to an ambient file.
///
/// # Errors
/// As for [`load`], for whichever files exist.
pub fn load_effective(start: &Path) -> Result<Option<Config>> {
    if env::var_os(CONFIG_ENV).is_some() {
        return load_default();
    }
    let Some(project_path) = find_project_file(start) else {
        return load_default();
    };
    let project = load(&project_path)?.unwrap_or_default();
    Ok(Some(resolve_project(
        &project_path,
        project,
        load_default()?,
    )))
}

/// Writes a `.beagle` in `dir`: an empty file when `root` is `None` (the
/// directory itself becomes the store root), else a single `root = "..."`
/// assignment. A relative root is written verbatim — it resolves against
/// the `.beagle`'s directory by [`load_effective`]'s rules. Refuses to
/// overwrite an existing file.
///
/// # Errors
/// [`Error::AlreadyExists`] if a `.beagle` is already there,
/// [`Error::Io`] on write failures.
pub fn write_project_file(dir: &Path, root: Option<&Path>) -> Result<PathBuf> {
    let path = dir.join(PROJECT_FILE);
    if path.exists() {
        return Err(Error::AlreadyExists(path.display().to_string()));
    }
    let contents = match root {
        None => String::new(),
        Some(root) => format!("root = {}\n", toml::Value::from(root.display().to_string())),
    };
    fs::write(&path, &contents).map_err(|e| Error::io(&path, e))?;
    Ok(path)
}

/// The pure core of [`load_effective`]: pins the root from the project
/// file's location and fills unset fields from the global config.
pub(super) fn resolve_project(
    project_path: &Path,
    mut project: Config,
    global: Option<Config>,
) -> Config {
    let dir = project_path.parent().unwrap_or(Path::new("."));
    project.root = Some(match project.root.take() {
        Some(root) if root.is_relative() => dir.join(root),
        Some(root) => root,
        None => dir.to_owned(),
    });
    if let Some(global) = global {
        project.editor = project.editor.or(global.editor);
        project.notify = project.notify.or(global.notify);
        project.notify_events = project.notify_events.or(global.notify_events);
        project.handoff = project.handoff.or(global.handoff);
        project.tickets = project.tickets.or(global.tickets);
    }
    project
}
