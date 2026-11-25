//! macOS notifications for project and task events, sent by `beagle scout`
//! at the moment an agent changes a status.
//!
//! Same philosophy as beagle's: shell out, fire and forget, never fail.
//! Banners come from scout's own `Scout.app` (a dog-iconed copy of
//! `terminal-notifier`, see [`crate::notify_app`]) when it is set up, else
//! plain `terminal-notifier`. Either way, clicking a notification runs
//! `herdr agent focus <pane>` and brings the terminal app forward, so you
//! land in the agent's pane. Otherwise it falls back to a plain `osascript`
//! banner. Off on other platforms, and whenever `SCOUT_NOTIFY=0`.

use beagle_utils::shell::quote;
use std::process::Command;

use crate::config::Config;

/// What happened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// An agent started working a project.
    ProjectStarted {
        /// The task it started with.
        first_task: String,
    },
    /// A project was marked done.
    ProjectDone {
        /// How many tasks it closed.
        tasks: usize,
    },
    /// A task was blocked: the agent needs the user.
    Blocked {
        /// `NN-slug · title`.
        task: String,
        /// What is needed, if the agent said.
        reason: Option<String>,
    },
    /// A task was finished.
    TaskDone {
        /// `NN-slug · title`.
        task: String,
        /// `(closed, total)` after this task.
        progress: (usize, usize),
        /// The next open task, if any.
        next: Option<String>,
    },
    /// A task was started.
    TaskStarted {
        /// `NN-slug · title`.
        task: String,
    },
    /// A working task went quiet and was paused automatically.
    TaskPaused {
        /// `NN-slug · title`.
        task: String,
        /// How long it had been quiet, e.g. `34m`.
        quiet: String,
    },
}

/// One notification, ready to send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    /// Bold first line.
    pub title: String,
    /// Second line.
    pub subtitle: String,
    /// Body.
    pub message: String,
    /// Sound to play, if any.
    pub sound: Option<String>,
    /// Notifications in the same group replace each other.
    pub group: String,
}

/// Where the agent runs, so a click can take the user there.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Origin {
    /// The agent's Herdr pane (`HERDR_PANE_ID`).
    pub pane: Option<String>,
    /// The `herdr` binary (`HERDR_BIN_PATH`, else `herdr`).
    pub herdr: Option<String>,
    /// The terminal app's bundle id (`__CFBundleIdentifier`), to activate.
    pub app: Option<String>,
}

impl Origin {
    /// Reads the origin from the environment of the agent's `beagle scout` call.
    #[must_use]
    pub fn from_env() -> Self {
        let var = |name| std::env::var(name).ok().filter(|v: &String| !v.is_empty());
        Self {
            pane: var("HERDR_PANE_ID"),
            herdr: var("HERDR_BIN_PATH"),
            app: var("__CFBundleIdentifier"),
        }
    }
}

/// Builds the notification for `event` in `project`, or `None` when the
/// config turns this event (or notifications) off.
#[must_use]
pub fn build(
    config: &Config,
    project_id: &str,
    project_title: &str,
    event: &Event,
) -> Option<Notification> {
    let on = &config.notify_events;
    let enabled = config.notify
        && match event {
            Event::ProjectStarted { .. } => on.project_started,
            Event::ProjectDone { .. } => on.project_done,
            Event::Blocked { .. } => on.blocked,
            Event::TaskDone { .. } => on.task_done,
            Event::TaskStarted { .. } => on.task_started,
            Event::TaskPaused { .. } => on.task_paused,
        };
    enabled.then(|| render(config, project_id, project_title, event))
}

/// The notification for `event` whatever the config switches say: the
/// banner's wording, also written to the event spool for the phone
/// ([`crate::events`]). Only the sound comes from `config`.
#[must_use]
pub fn render(
    config: &Config,
    project_id: &str,
    project_title: &str,
    event: &Event,
) -> Notification {
    let (subtitle, message, sound) = match event {
        Event::ProjectStarted { first_task } => (
            format!("▶ Started: {project_title}"),
            format!("First task: {first_task}"),
            None,
        ),
        Event::ProjectDone { tasks } => (
            format!("✔ Project done: {project_title}"),
            format!("{tasks} task(s) closed"),
            None,
        ),
        Event::Blocked { task, reason } => (
            format!("■ Needs you: {task}"),
            reason
                .clone()
                .unwrap_or_else(|| "The agent is blocked and waiting on you.".to_owned()),
            Some(config.sound.clone()).filter(|s| !s.is_empty()),
        ),
        Event::TaskDone {
            task,
            progress: (closed, total),
            next,
        } => (
            format!("✔ Done: {task} ({closed}/{total})"),
            next.as_ref().map_or_else(
                || "All tasks closed.".to_owned(),
                |next| format!("Next: {next}"),
            ),
            None,
        ),
        Event::TaskPaused { task, quiet } => (
            format!("‖ Paused: {task}"),
            format!("Quiet for {quiet}: the agent may have stalled or finished without saying."),
            None,
        ),
        Event::TaskStarted { task } => {
            (format!("▶ Working: {task}"), project_title.to_owned(), None)
        }
    };
    Notification {
        title: format!("Scout · {project_id}"),
        subtitle,
        message,
        sound,
        group: format!("scout-{project_id}"),
    }
}

/// The command that shows `n` on this machine, or `None` where there is no
/// notifier. `notifier` is a terminal-notifier executable (scout's own
/// `Scout.app` when set up, so the banner shows the dog), or `None` for a
/// plain `osascript` banner. Split from [`send`] so it is testable without
/// popping banners.
#[must_use]
pub fn command(
    n: &Notification,
    origin: &Origin,
    notifier: Option<&str>,
) -> Option<(String, Vec<String>)> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    if let Some(notifier) = notifier {
        let mut args = vec![
            "-title".to_owned(),
            n.title.clone(),
            "-subtitle".to_owned(),
            n.subtitle.clone(),
            "-message".to_owned(),
            n.message.clone(),
            "-group".to_owned(),
            n.group.clone(),
        ];
        if let Some(sound) = &n.sound {
            args.extend(["-sound".to_owned(), sound.clone()]);
        }
        if let Some(pane) = &origin.pane {
            let herdr = origin.herdr.as_deref().unwrap_or("herdr");
            args.extend([
                "-execute".to_owned(),
                format!("{} agent focus {}", quote(herdr), quote(pane)),
            ]);
        }
        if let Some(app) = &origin.app {
            args.extend(["-activate".to_owned(), app.clone()]);
        }
        return Some((notifier.to_owned(), args));
    }
    let mut script = format!(
        "display notification {} with title {} subtitle {}",
        applescript_string(&n.message),
        applescript_string(&n.title),
        applescript_string(&n.subtitle),
    );
    if let Some(sound) = &n.sound {
        script.push_str(" sound name ");
        script.push_str(&applescript_string(sound));
    }
    Some(("osascript".to_owned(), vec!["-e".to_owned(), script]))
}

/// Shows `n`, fire and forget. Every failure is silent: a notification is a
/// convenience and must never break the agent's command.
pub fn send(n: &Notification, origin: &Origin) {
    if std::env::var("SCOUT_NOTIFY").is_ok_and(|v| v == "0") {
        return;
    }
    let scout_app =
        beagle_utils::paths::home_dir().and_then(|home| crate::notify_app::executable(&home));
    let notifier = scout_app
        .map(|p| p.to_string_lossy().into_owned())
        .or_else(|| on_path("terminal-notifier").then(|| "terminal-notifier".to_owned()));
    let Some((program, args)) = command(n, origin, notifier.as_deref()) else {
        return;
    };
    // Fire and forget: a banner that fails to show is not worth reporting.
    // Bounded so a stuck notifier is killed and reaped, never left behind.
    let _ = beagle_utils::process::spawn_bounded(
        Command::new(program).args(args),
        std::time::Duration::from_secs(10),
    );
}

/// Whether `program` is on this process's PATH (the one `Command::new`
/// searches).
fn on_path(program: &str) -> bool {
    let path = std::env::var_os("PATH").unwrap_or_default();
    beagle_utils::process::which_in(program, &path).is_some()
}

/// An `AppleScript` string literal.
fn applescript_string(s: &str) -> String {
    format!("\"{}\"", beagle_utils::notify::applescript_escape(s))
}

#[cfg(test)]
// Tests fail loudly on setup errors; unwrap/expect/panic is the clearest way.
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::config::NotifyEvents;

    fn blocked() -> Event {
        Event::Blocked {
            task: "03-open-pr · Open the PR".into(),
            reason: Some("Push branch fix-retry and open a PR?".into()),
        }
    }

    #[test]
    fn defaults_notify_blocked_and_project_events_only() {
        let config = Config::default();
        let n = build(&config, "p", "Rate-limit the API", &blocked()).unwrap();
        assert_eq!(n.subtitle, "■ Needs you: 03-open-pr · Open the PR");
        assert_eq!(n.message, "Push branch fix-retry and open a PR?");
        assert_eq!(n.sound.as_deref(), Some("Glass"));
        assert_eq!(n.group, "scout-p");
        assert_eq!(n.title, "Scout · p");
        assert!(build(
            &config,
            "p",
            "T",
            &Event::ProjectStarted {
                first_task: "01".into()
            }
        )
        .is_some());
        assert!(build(&config, "p", "T", &Event::ProjectDone { tasks: 3 }).is_some());
        assert!(build(&config, "p", "T", &Event::TaskStarted { task: "01".into() }).is_none());
        let done = Event::TaskDone {
            task: "01".into(),
            progress: (1, 3),
            next: None,
        };
        assert!(build(&config, "p", "T", &done).is_none());
    }

    #[test]
    fn config_switches_events() {
        let config = Config {
            notify_events: NotifyEvents {
                task_done: true,
                blocked: false,
                ..NotifyEvents::default()
            },
            ..Config::default()
        };
        let done = Event::TaskDone {
            task: "01 · A".into(),
            progress: (1, 3),
            next: Some("02 · B".into()),
        };
        let n = build(&config, "p", "T", &done).unwrap();
        assert_eq!(
            (n.subtitle.as_str(), n.message.as_str()),
            ("✔ Done: 01 · A (1/3)", "Next: 02 · B")
        );
        assert!(build(&config, "p", "T", &blocked()).is_none());
        let off = Config {
            notify: false,
            ..Config::default()
        };
        assert!(build(&off, "p", "T", &blocked()).is_none());
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn click_focuses_the_agent_pane() {
        let n = build(&Config::default(), "p", "T", &blocked()).unwrap();
        let origin = Origin {
            pane: Some("wC:p1".into()),
            herdr: Some("/opt/homebrew/bin/herdr".into()),
            app: Some("com.mitchellh.ghostty".into()),
        };
        let app = "/Users/me/Library/Application Support/scout/Scout.app/Contents/MacOS/terminal-notifier";
        let (program, args) = command(&n, &origin, Some(app)).unwrap();
        assert_eq!(program, app);
        let joined = args.join(" ");
        assert!(
            joined.contains("-execute '/opt/homebrew/bin/herdr' agent focus 'wC:p1'"),
            "{joined}"
        );
        assert!(joined.contains("-activate com.mitchellh.ghostty"));
        assert!(joined.contains("-sound Glass"));
        let (program, args) = command(&n, &Origin::default(), None).unwrap();
        assert_eq!(program, "osascript");
        assert!(args[1].starts_with("display notification \"Push branch"));
        assert!(args[1].ends_with("sound name \"Glass\""));
    }

    #[test]
    fn applescript_escaping() {
        assert_eq!(
            applescript_string(r#"say "hi" \o/"#),
            r#""say \"hi\" \\o/""#
        );
    }
}
