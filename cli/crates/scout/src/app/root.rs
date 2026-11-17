//! Which store a command uses.
//!
//! First match wins:
//!
//! 1. `--root <dir>`
//! 2. `$SCOUT_ROOT`
//! 3. the nearest directory at or above the current one holding a
//!    [`MARKER`] file — a store you keep in its own repo, used whenever
//!    you are inside it
//! 4. `root` in `~/.config/scout/config.toml` — your default store, used
//!    from anywhere, which is what agents working in other repos need
//! 5. `~/.scout`
//!
//! The marker is a FILE on purpose: `~/.scout` itself is a directory, and a
//! directory marker would make every path under `$HOME` resolve to `$HOME`.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

/// The file that marks a directory as a scout store (`beagle scout init`).
pub(crate) const MARKER: &str = ".scout-store";

/// Everything resolution reads, gathered so the rule itself is pure.
#[derive(Debug, Default)]
pub(crate) struct Inputs {
    pub explicit: Option<PathBuf>,
    pub env: Option<PathBuf>,
    pub cwd: Option<PathBuf>,
    pub config_root: Option<PathBuf>,
    pub home: Option<PathBuf>,
}

/// Where the store came from, for `beagle scout root`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Source {
    Flag,
    Env,
    Marker,
    Config,
    Default,
}

impl Source {
    pub(crate) fn describe(self) -> &'static str {
        match self {
            Self::Flag => "--root",
            Self::Env => "$SCOUT_ROOT",
            Self::Marker => "nearest .scout-store",
            Self::Config => "root in ~/.config/scout/config.toml",
            Self::Default => "default",
        }
    }
}

/// The store root and where it came from; `None` only with no home dir
/// and nothing else to go on.
pub(crate) fn resolve(inputs: Inputs) -> Option<(PathBuf, Source)> {
    if let Some(root) = inputs.explicit {
        return Some((root, Source::Flag));
    }
    if let Some(root) = inputs.env {
        return Some((root, Source::Env));
    }
    if let Some(root) = inputs.cwd.as_deref().and_then(marked_ancestor) {
        return Some((root, Source::Marker));
    }
    if let Some(root) = inputs.config_root {
        return Some((root, Source::Config));
    }
    inputs.home.map(|h| (h.join(".scout"), Source::Default))
}

/// The nearest directory at or above `start` that holds a [`MARKER`] file.
pub(crate) fn marked_ancestor(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|dir| dir.join(MARKER).is_file())
        .map(Path::to_path_buf)
}

/// Paths a store must keep out of git: session recordings and their
/// rendered copies hold raw command output, which can include secrets.
const GITIGNORE_LINES: [&str; 2] = ["/.sessions/", "/*/sessions/"];

/// Marks `dir` as a store (creating it if needed) and makes sure its
/// `.gitignore` excludes session data. Idempotent; never removes a line.
pub(crate) fn init(dir: &Path) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let marker = dir.join(MARKER);
    if !marker.is_file() {
        std::fs::write(
            &marker,
            "# This directory is a scout store: `beagle scout` run anywhere\n\
             # inside it uses the projects here. Created by `beagle scout init`.\n",
        )?;
    }
    let ignore = dir.join(".gitignore");
    let existing = std::fs::read_to_string(&ignore).unwrap_or_default();
    let missing: Vec<&str> = GITIGNORE_LINES
        .into_iter()
        .filter(|line| !existing.lines().any(|l| l.trim() == *line))
        .collect();
    if !missing.is_empty() {
        let mut text = existing;
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str("# scout session recordings: raw command output, never commit\n");
        for line in missing {
            text.push_str(line);
            text.push('\n');
        }
        std::fs::write(&ignore, text)?;
    }
    Ok(marker)
}

/// The store in use from this process, and where it came from.
pub(super) fn current(explicit: Option<PathBuf>) -> Result<(PathBuf, Source)> {
    let config_root = crate::config::load()
        .0
        .root
        .map(|r| beagle_utils::paths::expand_tilde(&r));
    resolve(Inputs {
        explicit,
        env: beagle_utils::paths::env_path("SCOUT_ROOT"),
        cwd: std::env::current_dir().ok(),
        config_root,
        home: beagle_utils::paths::home_dir(),
    })
    .ok_or(Error::NoHome)
}

/// `beagle scout root`.
pub(super) fn print_current(explicit: Option<PathBuf>) -> Result<()> {
    let (path, source) = current(explicit)?;
    println!("{}\t({})", path.display(), source.describe());
    Ok(())
}

/// `beagle scout init [<dir>]`.
pub(super) fn init_command(dir: Option<PathBuf>) -> Result<()> {
    let dir = match dir {
        Some(d) => d,
        None => std::env::current_dir().map_err(|source| Error::Io {
            path: PathBuf::from("."),
            source,
        })?,
    };
    let marker = init(&dir).map_err(|source| Error::Io {
        path: dir.clone(),
        source,
    })?;
    println!(
        "{} is now a scout store ({})",
        dir.display(),
        marker.display()
    );
    Ok(())
}

#[cfg(test)]
// Tests fail loudly on setup errors; unwrap is the clearest way.
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn inputs(cwd: &Path) -> Inputs {
        Inputs {
            cwd: Some(cwd.to_path_buf()),
            config_root: Some(PathBuf::from("/cfg/store")),
            home: Some(PathBuf::from("/home/u")),
            ..Inputs::default()
        }
    }

    #[test]
    fn flag_then_env_beat_everything() {
        let dir = tempfile::tempdir().unwrap();
        init(dir.path()).unwrap();
        let mut i = inputs(dir.path());
        i.env = Some(PathBuf::from("/env"));
        i.explicit = Some(PathBuf::from("/flag"));
        assert_eq!(resolve(i).unwrap(), (PathBuf::from("/flag"), Source::Flag));
        let mut i = inputs(dir.path());
        i.env = Some(PathBuf::from("/env"));
        assert_eq!(resolve(i).unwrap(), (PathBuf::from("/env"), Source::Env));
    }

    #[test]
    fn a_marked_ancestor_beats_the_config_root() {
        let dir = tempfile::tempdir().unwrap();
        init(dir.path()).unwrap();
        let deep = dir.path().join("a/b");
        std::fs::create_dir_all(&deep).unwrap();
        let (root, source) = resolve(inputs(&deep)).unwrap();
        assert_eq!(root, dir.path());
        assert_eq!(source, Source::Marker);
    }

    #[test]
    fn outside_any_store_the_config_root_then_home_is_used() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            resolve(inputs(dir.path())).unwrap(),
            (PathBuf::from("/cfg/store"), Source::Config)
        );
        let mut i = inputs(dir.path());
        i.config_root = None;
        assert_eq!(
            resolve(i).unwrap(),
            (PathBuf::from("/home/u/.scout"), Source::Default)
        );
    }

    #[test]
    fn a_directory_named_like_the_marker_does_not_count() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join(MARKER)).unwrap();
        assert_eq!(marked_ancestor(dir.path()), None);
    }

    #[test]
    fn init_is_idempotent_and_creates_the_dir() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("new-store");
        let marker = init(&store).unwrap();
        assert!(marker.is_file());
        std::fs::write(&marker, "kept").unwrap();
        init(&store).unwrap();
        assert_eq!(std::fs::read_to_string(&marker).unwrap(), "kept");
    }

    #[test]
    fn init_keeps_session_data_out_of_git_without_duplicating() {
        let dir = tempfile::tempdir().unwrap();
        let ignore = dir.path().join(".gitignore");
        std::fs::write(&ignore, "target/\n/.sessions/").unwrap();
        init(dir.path()).unwrap();
        init(dir.path()).unwrap();
        let text = std::fs::read_to_string(&ignore).unwrap();
        assert!(text.starts_with("target/\n/.sessions/\n"));
        assert_eq!(text.matches("/.sessions/").count(), 1);
        assert_eq!(text.matches("/*/sessions/").count(), 1);
    }
}
