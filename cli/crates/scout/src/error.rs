//! The crate's single error type.

use std::path::PathBuf;

/// Everything that can go wrong in scout, with enough context to act on.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A filesystem operation failed.
    #[error("{}: {source}", .path.display())]
    Io {
        /// The file or directory involved.
        path: PathBuf,
        /// The underlying error.
        #[source]
        source: std::io::Error,
    },
    /// A file is bigger than scout is willing to read.
    #[error("{}: larger than 4 MB, refusing to read", .path.display())]
    TooLarge {
        /// The oversized file.
        path: PathBuf,
    },
    /// A project or task file has missing or invalid frontmatter.
    #[error("{}: {message}", .path.display())]
    Frontmatter {
        /// The file with the bad frontmatter.
        path: PathBuf,
        /// What is wrong with it.
        message: String,
    },
    /// A project or task id that breaks the slug rules.
    #[error(
        "invalid id {0:?}: use lowercase a-z, 0-9 and '-' (not at the start or end), at most 64 characters"
    )]
    InvalidSlug(String),
    /// No project with this id exists.
    #[error("no project {0:?} (see `beagle scout list`)")]
    ProjectNotFound(String),
    /// A project with this id already exists.
    #[error("project {0:?} already exists")]
    ProjectExists(String),
    /// No task matches this reference in the project.
    #[error("no task {task:?} in project {project:?} (see `beagle scout show {project}`)")]
    TaskNotFound {
        /// The project searched.
        project: String,
        /// The task reference that matched nothing.
        task: String,
    },
    /// A task with this slug already exists in the project.
    #[error("task {0:?} already exists in this project")]
    TaskExists(String),
    /// A session id prefix matches more than one recorded session.
    #[error("{0:?} matches more than one session: give more characters, or use --pick")]
    AmbiguousSession(String),
    /// The terminal could not be set up, drawn to, or read from.
    #[error("terminal error: {0}")]
    Terminal(#[source] std::io::Error),
    /// The filesystem watcher could not be attached.
    #[error("file watcher: {0}")]
    Watch(#[from] notify::Error),
    /// JSON output could not be produced.
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    /// `task attach` was given no session and is not running inside Claude
    /// Code.
    #[error("no session: run this from inside Claude Code, or pass --session <id>")]
    NoSession,
    /// No store root could be derived.
    #[error("no home directory: set BEAGLE_SCOUT_ROOT or pass --root")]
    NoHome,
    /// The editor could not be launched.
    #[error("could not run the editor {command:?}: {source}")]
    Editor {
        /// The editor command line.
        command: String,
        /// Why it failed to start.
        #[source]
        source: std::io::Error,
    },
}

/// The crate-wide result type.
pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    /// Builds a mapper for `map_err` that attaches `path` to an I/O error.
    pub(crate) fn io(path: impl Into<PathBuf>) -> impl FnOnce(std::io::Error) -> Self {
        let path = path.into();
        move |source| Self::Io { path, source }
    }
}
