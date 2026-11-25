//! Auto-pause: a `working` task nobody has touched for `[auto] pause_after`
//! (config, default 30 minutes) becomes `paused`, with a log line and a
//! notification. It means the agent stalled, died, or moved on without
//! saying so. An agent narrating the task again (`beagle scout log --task`) or
//! starting it again sets it back to `working`.
//!
//! "Touched" is the newest of: the task's file, the project's log, and the
//! transcript of every Claude session attached to the task (so an agent
//! that is thinking or reading still counts as active). One command that
//! runs longer than `pause_after` with no output will pause the task until
//! the agent next touches it.
//!
//! Run by the TUI's background thread (every 30 seconds) and by
//! `beagle scout list` / `beagle scout show`.

use std::path::Path;
use std::time::SystemTime;

use crate::config::Config;
use crate::events;
use crate::notify;
use crate::store::{self, AutoPaused, Project, Store, Task};
use crate::transcript;

/// Pauses quiet working tasks (when `pause_after` is on) and notifies.
/// Never fails: problems mean nothing is paused this time.
#[allow(clippy::must_use_candidate)] // callers mostly want the side effects
pub fn sweep(store: &Store, config: &Config, home: Option<&Path>) -> Vec<AutoPaused> {
    let Some(after) = config.auto.pause_after() else {
        return Vec::new();
    };
    let activity = |project: &Project, task: &Task| last_activity(project, task, home);
    let paused = store
        .pause_quiet_tasks(after, activity, SystemTime::now())
        .unwrap_or_default();
    for p in &paused {
        let event = notify::Event::TaskPaused {
            task: p.task.clone(),
            quiet: p.quiet.clone(),
        };
        events::emit(&events::record(
            (&p.project_id, &p.project_title),
            Some((&p.task_id, &p.task_title)),
            &event,
            time::OffsetDateTime::now_utc(),
        ));
        if let Some(n) = notify::build(config, &p.project_id, &p.project_title, &event) {
            notify::send(&n, &notify::Origin::default());
        }
    }
    paused
}

/// The newest sign of life for `task`.
fn last_activity(project: &Project, task: &Task, home: Option<&Path>) -> Option<SystemTime> {
    let mut last = task.modified.max(store::modified(&project.log_file()));
    for session in &task.meta.sessions {
        let transcript = home.and_then(|h| transcript::find_transcript(h, session));
        if let Some(path) = transcript {
            last = last.max(store::modified(&path));
        }
    }
    last
}

#[cfg(test)]
// Tests fail loudly on setup errors; unwrap/expect/panic is the clearest way.
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::model::{Slug, TaskStatus};

    #[test]
    fn off_means_never() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        store
            .create_project(&Slug::parse("p").unwrap(), "P", vec![])
            .unwrap();
        store
            .add_task("p", &Slug::parse("a").unwrap(), "A", vec![])
            .unwrap();
        store
            .set_task_status("p", "1", TaskStatus::Working)
            .unwrap();
        let mut config = Config::default();
        config.auto.pause_after = "off".to_owned();
        assert_eq!(sweep(&store, &config, None).len(), 0);
        config.auto.pause_after = "30m".to_owned();
        assert_eq!(sweep(&store, &config, None).len(), 0, "just touched");
        let p = store.project("p").unwrap();
        assert_eq!(p.tasks[0].meta.status, TaskStatus::Working);
    }
}
