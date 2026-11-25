//! Status changes and the notifications they trigger, plus `notify`.

use crate::config::{self, Config, NotifyEvents};
use crate::events;
use crate::model::{ProjectStatus, TaskStatus};
use crate::notify;
use crate::store::{Project, Store, Task};
use crate::Result;

use super::{claude_session, home, short};

/// `beagle scout task start|done|block|skip|status`, plus the notification
/// it triggers.
pub(super) fn set_task(
    store: &Store,
    project: &str,
    reference: &str,
    status: TaskStatus,
    reason: Option<&str>,
) -> Result<()> {
    let was_planning = store.project(project)?.meta.status == ProjectStatus::Planning;
    let (task, previous) = store.set_task_status(project, reference, status)?;
    println!("{}: {} → {}", task.id, previous.as_str(), status.as_str());
    // Starting work attaches the agent's own Claude session, so the TUI's
    // Session tab shows it without the agent doing anything else.
    if status == TaskStatus::Working {
        if let Some(session) = claude_session() {
            if store.attach_session(project, &task.id, &session)? {
                println!(
                    "{}: attached this Claude session ({})",
                    task.id,
                    short(&session)
                );
            }
        }
    }
    if let Some(reason) = reason {
        store.append_log(
            project,
            Some(&task.id),
            &format!("{}: {reason}", status.as_str()),
        )?;
    }
    let p = store.project(project)?;
    if status == TaskStatus::Working {
        let others: Vec<_> = p
            .working()
            .filter(|t| t.id != task.id)
            .map(|t| t.id.clone())
            .collect();
        if !others.is_empty() {
            eprintln!("note: also working: {}", others.join(", "));
        }
    }
    let label = |t: &Task| format!("{} · {}", t.id, t.meta.title);
    let event = match status {
        TaskStatus::Working if was_planning => Some(notify::Event::ProjectStarted {
            first_task: label(&task),
        }),
        TaskStatus::Working => Some(notify::Event::TaskStarted { task: label(&task) }),
        TaskStatus::Blocked => Some(notify::Event::Blocked {
            task: label(&task),
            reason: reason.map(str::to_owned),
        }),
        TaskStatus::Done => Some(notify::Event::TaskDone {
            task: label(&task),
            progress: (p.closed_count(), p.tasks.len()),
            next: p
                .tasks
                .iter()
                .find(|t| !t.meta.status.is_closed())
                .map(label),
        }),
        TaskStatus::Todo | TaskStatus::Paused | TaskStatus::Skipped => None,
    };
    if let Some(event) = event {
        notify_event(&p, Some(&task), &event);
    }
    Ok(())
}

/// `beagle scout status <project> <status>`, plus the notification it
/// triggers.
pub(super) fn set_project(store: &Store, project: &str, status: ProjectStatus) -> Result<()> {
    let previous = store.set_project_status(project, status)?;
    println!("{project}: {} → {}", previous.as_str(), status.as_str());
    let p = store.project(project)?;
    let event = match status {
        ProjectStatus::Active if previous == ProjectStatus::Planning => {
            Some(notify::Event::ProjectStarted {
                first_task: p.next_todo().map_or_else(
                    || "-".to_owned(),
                    |t| format!("{} · {}", t.id, t.meta.title),
                ),
            })
        }
        ProjectStatus::Done if previous != ProjectStatus::Done => {
            Some(notify::Event::ProjectDone {
                tasks: p.closed_count(),
            })
        }
        ProjectStatus::Planning
        | ProjectStatus::Active
        | ProjectStatus::Paused
        | ProjectStatus::Done
        | ProjectStatus::Dropped => None,
    };
    if let Some(event) = event {
        notify_event(&p, None, &event);
    }
    Ok(())
}

/// Shows the macOS banner for `event` (when the config wants one) and
/// writes it to the event spool for the phone (always).
fn notify_event(project: &Project, task: Option<&Task>, event: &notify::Event) {
    events::emit(&events::record(
        (project.id.as_str(), &project.meta.title),
        task.map(|t| (t.id.as_str(), t.meta.title.as_str())),
        event,
        time::OffsetDateTime::now_utc(),
    ));
    let (config, warning) = config::load();
    if let Some(warning) = warning {
        eprintln!("warning: {warning}");
    }
    if let Some(n) = notify::build(&config, project.id.as_str(), &project.meta.title, event) {
        notify::send(&n, &notify::Origin::from_env());
    }
}

/// `beagle scout notify setup`: build `Scout.app` so banners show the dog.
pub(super) fn notify_setup() -> Result<()> {
    let app = crate::notify_app::setup(&home()?)?;
    println!("built {}", app.display());
    println!("  notifications now come from \"Scout\" with the dog icon");
    println!("  (macOS may ask once whether to allow notifications from Scout)");
    Ok(())
}

/// `beagle scout notify test`: one sample "needs you" banner, plus where
/// the settings live.
pub(super) fn notify_test() {
    let (config, warning) = config::load();
    let path = config::path().map_or_else(|| "-".to_owned(), |p| p.display().to_string());
    if let Some(warning) = warning {
        eprintln!("warning: {warning}");
    }
    println!("settings: {path}");
    println!(
        "notify = {}  ·  events: {:?}",
        config.notify, config.notify_events
    );
    let config = Config {
        notify: true,
        notify_events: NotifyEvents {
            blocked: true,
            ..config.notify_events
        },
        ..config
    };
    let event = notify::Event::Blocked {
        task: "03-open-pr · Open the PR".to_owned(),
        reason: Some("Test notification from `beagle scout notify test`.".to_owned()),
    };
    if let Some(n) = notify::build(&config, "example", "Example project", &event) {
        notify::send(&n, &notify::Origin::from_env());
        println!("sent a sample notification (click it to focus this Herdr pane)");
    }
}
