//! The session state: one value per file in a directory, like the script.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Every file a session writes besides `startup.log`. `off` removes them;
/// the log stays for `log`.
pub(crate) const SESSION_FILES: [&str; 6] = [
    "caffeinate.pid",
    "timer.pid",
    "startup.pid",
    "since",
    "until",
    "file",
];

/// The startup file's output.
pub(crate) const LOG_FILE: &str = "startup.log";

/// A state directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StateDir {
    /// Where the files live.
    pub dir: PathBuf,
}

impl StateDir {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    /// The trimmed value of `name`, or `None` when missing or empty.
    pub fn read(&self, name: &str) -> Option<String> {
        let text = fs::read_to_string(self.path(name)).ok()?;
        let text = text.trim();
        (!text.is_empty()).then(|| text.to_owned())
    }

    pub fn read_i64(&self, name: &str) -> Option<i64> {
        self.read(name)?.parse().ok()
    }

    pub fn read_pid(&self, name: &str) -> Option<u32> {
        self.read(name)?.parse().ok().filter(|&p| p > 0)
    }

    pub fn write(&self, name: &str, value: impl std::fmt::Display) -> io::Result<()> {
        fs::create_dir_all(&self.dir)?;
        fs::write(self.path(name), format!("{value}\n"))
    }

    /// Removes every [`SESSION_FILES`] entry; missing ones are fine.
    pub fn clear(&self) {
        for name in SESSION_FILES {
            let _ = fs::remove_file(self.path(name));
        }
    }

    pub fn has_log(&self) -> bool {
        self.path(LOG_FILE).is_file()
    }
}

/// `$XDG_STATE_HOME/beagle/server-mode` (default
/// `~/.local/state/beagle/server-mode`).
pub(crate) fn default_dir() -> Option<PathBuf> {
    beagle_utils::paths::xdg_state_home().map(|d| d.join("beagle").join("server-mode"))
}

/// Where the old standalone script kept its state: always
/// `~/.local/state/server-mode`, whatever `$XDG_STATE_HOME` says.
pub(crate) fn legacy_dir(home: &Path) -> PathBuf {
    home.join(".local").join("state").join("server-mode")
}

/// The last `n` lines of `text`.
pub(crate) fn tail(text: &str, n: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(n);
    let mut out = lines[start..].join("\n");
    if !out.is_empty() {
        out.push('\n');
    }
    out
}

/// `1h05m` for 3900 seconds: the script's `fmt_dur`. Negative clamps to 0.
pub(crate) fn fmt_dur(secs: i64) -> String {
    let secs = secs.max(0);
    format!("{}h{:02}m", secs / 3600, secs % 3600 / 60)
}
