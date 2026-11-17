//! User settings: `~/.config/scout/config.toml` (or `$XDG_CONFIG_HOME`).
//!
//! ```toml
//! notify = true                 # macOS notifications from the scout CLI
//!
//! [notify_events]               # which events notify (these are the defaults)
//! blocked = true                # a task is blocked: the agent needs you (plays a sound)
//! project_started = true        # an agent starts working a project
//! project_done = true           # a project is marked done
//! task_done = false             # every finished task
//! task_started = false          # every task an agent picks up (noisy)
//!
//! task_paused = true            # a working task went quiet and was auto-paused
//!
//! sound = "Glass"               # the sound for `blocked`; "" for none
//!
//! [auto]
//! pause_after = "30m"           # quiet this long → a working task becomes paused;
//!                               # "45s", "30m", "2h", or "off"
//! ```
//!
//! Every key is optional. A missing file means the defaults above. A broken
//! file is reported as a warning and the defaults are used, so a typo can
//! never make an agent's `beagle scout` command fail.

use std::path::{Path, PathBuf};

use serde::Deserialize;

/// The parsed settings, with defaults filled in.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Send notifications at all.
    pub notify: bool,
    /// Which events notify.
    pub notify_events: NotifyEvents,
    /// The sound played for `blocked` notifications; empty for none.
    pub sound: String,
    /// Automatic status changes.
    pub auto: Auto,
}

/// Automatic status changes.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Auto {
    /// How long a `working` task may be quiet before it becomes `paused`:
    /// `"45s"`, `"30m"`, `"2h"`, or `"off"`.
    pub pause_after: String,
}

impl Default for Auto {
    fn default() -> Self {
        Self {
            pause_after: "30m".to_owned(),
        }
    }
}

impl Auto {
    /// [`Auto::pause_after`] as a duration; `None` when off or unreadable.
    #[must_use]
    pub fn pause_after(&self) -> Option<std::time::Duration> {
        parse_duration(&self.pause_after)
    }
}

/// `"45s"`, `"30m"`, `"2h"`, `"1d"` (or a bare number of minutes). `None`
/// for `"off"`, `"0"`, empty, or anything unreadable.
#[must_use]
pub fn parse_duration(raw: &str) -> Option<std::time::Duration> {
    let raw = raw.trim();
    let (number, unit) = raw
        .find(|c: char| !c.is_ascii_digit())
        .map_or((raw, "m"), |i| raw.split_at(i));
    let n: u64 = number.parse().ok().filter(|n| *n > 0)?;
    let secs = match unit.trim() {
        "s" => n,
        "m" => n * 60,
        "h" => n * 3600,
        "d" => n * 86_400,
        _ => return None,
    };
    Some(std::time::Duration::from_secs(secs))
}

/// Which events send a notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)] // one independent switch per event
pub struct NotifyEvents {
    /// A task was blocked (the agent needs you).
    pub blocked: bool,
    /// A project's first task started (or it was set active from planning).
    pub project_started: bool,
    /// A project was marked done.
    pub project_done: bool,
    /// A task was marked done.
    pub task_done: bool,
    /// A task was started.
    pub task_started: bool,
    /// A working task went quiet and was paused automatically.
    pub task_paused: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            notify: true,
            notify_events: NotifyEvents::default(),
            sound: "Glass".to_owned(),
            auto: Auto::default(),
        }
    }
}

impl Default for NotifyEvents {
    fn default() -> Self {
        Self {
            blocked: true,
            project_started: true,
            project_done: true,
            task_done: false,
            task_started: false,
            task_paused: true,
        }
    }
}

/// Where the config file lives: `$XDG_CONFIG_HOME/scout/config.toml`, else
/// `~/.config/scout/config.toml`.
#[must_use]
pub fn path() -> Option<PathBuf> {
    beagle_utils::paths::xdg_config_home().map(|base| base.join("scout").join("config.toml"))
}

/// Loads the config, falling back to defaults. The second value is a
/// warning to show when the file exists but could not be used.
#[must_use]
pub fn load() -> (Config, Option<String>) {
    match path() {
        Some(path) => load_from(&path),
        None => (Config::default(), None),
    }
}

/// [`load`] for a specific file.
#[must_use]
pub fn load_from(path: &Path) -> (Config, Option<String>) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return (Config::default(), None); // no file: defaults, silently
    };
    match toml::from_str(&text) {
        Ok(config) => (config, None),
        Err(e) => (
            Config::default(),
            Some(format!(
                "{}: {} (using defaults)",
                path.display(),
                e.message()
            )),
        ),
    }
}

#[cfg(test)]
// Tests fail loudly on setup errors; unwrap/expect/panic is the clearest way.
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_is_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let (config, warning) = load_from(&dir.path().join("nope.toml"));
        assert_eq!(config, Config::default());
        assert!(
            config.notify && config.notify_events.blocked && !config.notify_events.task_started
        );
        assert!(warning.is_none());
    }

    #[test]
    fn partial_file_keeps_other_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "sound = \"\"\n[notify_events]\ntask_done = true\n").unwrap();
        let (config, warning) = load_from(&path);
        assert!(warning.is_none());
        assert!(config.notify);
        assert!(config.notify_events.task_done);
        assert!(
            config.notify_events.blocked,
            "untouched keys keep their default"
        );
        assert_eq!(config.sound, "");
    }

    #[test]
    fn durations() {
        use std::time::Duration;
        assert_eq!(parse_duration("45s"), Some(Duration::from_secs(45)));
        assert_eq!(parse_duration("30m"), Some(Duration::from_mins(30)));
        assert_eq!(parse_duration("2h"), Some(Duration::from_secs(7200)));
        assert_eq!(
            parse_duration("15"),
            Some(Duration::from_mins(15)),
            "bare = minutes"
        );
        for off in ["off", "0", "", "soon", "5y"] {
            assert_eq!(parse_duration(off), None, "{off:?}");
        }
        assert_eq!(
            Config::default().auto.pause_after(),
            Some(Duration::from_mins(30))
        );
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "[auto]\npause_after = \"off\"\n").unwrap();
        assert_eq!(load_from(&path).0.auto.pause_after(), None);
    }

    #[test]
    fn typos_warn_and_fall_back() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "notfy = false\n").unwrap();
        let (config, warning) = load_from(&path);
        assert_eq!(config, Config::default());
        assert!(warning.unwrap().contains("notfy"));
    }
}
