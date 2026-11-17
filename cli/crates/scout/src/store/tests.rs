//! Store tests: create, list, mutate and the log.

// Tests fail loudly on setup errors; unwrap/expect/panic is the clearest way.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;

fn store() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path().join("scout"));
    (dir, store)
}

fn slug(s: &str) -> Slug {
    Slug::parse(s).unwrap()
}

#[test]
fn missing_root_lists_nothing() {
    let (_dir, store) = store();
    let listing = store.list().unwrap();
    assert_eq!(listing.projects.len(), 0);
    assert_eq!(listing.warnings.len(), 0);
}

#[test]
fn create_project_scaffolds_files() {
    let (_dir, store) = store();
    let p = store
        .create_project(&slug("alpha"), "Alpha goal", vec!["ABC-1".into()])
        .unwrap();
    assert_eq!(p.meta.status, ProjectStatus::Planning);
    assert_eq!(p.meta.links, ["ABC-1"]);
    assert!(p.file().is_file());
    assert!(p.dir.join(TASKS_DIR).is_dir());
    let log = fs::read_to_string(p.log_file()).unwrap();
    assert!(log.contains("project created: Alpha goal"), "{log}");
    let body = read_capped(&p.file()).unwrap();
    assert!(body.contains("## How this part of the stack works"));
    assert!(matches!(
        store.create_project(&slug("alpha"), "again", vec![]),
        Err(Error::ProjectExists(_))
    ));
}

#[test]
fn tasks_number_in_order_and_resolve() {
    let (_dir, store) = store();
    store.create_project(&slug("p"), "P", vec![]).unwrap();
    let a = store
        .add_task("p", &slug("first"), "First", vec![])
        .unwrap();
    let b = store
        .add_task("p", &slug("second"), "Second", vec![])
        .unwrap();
    assert_eq!(a.id, "01-first");
    assert_eq!(b.id, "02-second");
    assert_eq!(a.checklist, (0, 2));
    let p = store.project("p").unwrap();
    assert_eq!(p.resolve_task("2").unwrap().id, "02-second");
    assert_eq!(p.resolve_task("02").unwrap().id, "02-second");
    assert_eq!(p.resolve_task("01-first").unwrap().id, "01-first");
    assert_eq!(p.resolve_task("first").unwrap().id, "01-first");
    assert!(p.resolve_task("3").is_none());
    assert!(matches!(
        store.add_task("p", &slug("first"), "dup", vec![]),
        Err(Error::TaskExists(_))
    ));
}

#[test]
fn numbering_continues_after_removal() {
    let (_dir, store) = store();
    store.create_project(&slug("p"), "P", vec![]).unwrap();
    store.add_task("p", &slug("a"), "A", vec![]).unwrap();
    store.add_task("p", &slug("b"), "B", vec![]).unwrap();
    store.remove_task("p", "1").unwrap();
    let c = store.add_task("p", &slug("c"), "C", vec![]).unwrap();
    assert_eq!(c.id, "03-c");
    assert_eq!(store.project("p").unwrap().tasks.len(), 2);
}

#[test]
fn working_activates_a_planning_project_and_logs() {
    let (_dir, store) = store();
    store.create_project(&slug("p"), "P", vec![]).unwrap();
    store.add_task("p", &slug("a"), "A", vec![]).unwrap();
    let (task, previous) = store
        .set_task_status("p", "1", TaskStatus::Working)
        .unwrap();
    assert_eq!(previous, TaskStatus::Todo);
    assert_eq!(task.meta.status, TaskStatus::Working);
    assert!(task.meta.updated.is_some());
    let p = store.project("p").unwrap();
    assert_eq!(p.meta.status, ProjectStatus::Active);
    assert_eq!(p.working().count(), 1);
    let log = fs::read_to_string(p.log_file()).unwrap();
    assert!(log.contains("`01-a` todo → working"), "{log}");
    assert!(log.contains("project planning → active"), "{log}");
}

#[test]
fn status_change_keeps_the_body() {
    let (_dir, store) = store();
    store.create_project(&slug("p"), "P", vec![]).unwrap();
    let t = store.add_task("p", &slug("a"), "A", vec![]).unwrap();
    let before = frontmatter::body(&read_capped(&t.path).unwrap()).to_owned();
    store.set_task_status("p", "a", TaskStatus::Done).unwrap();
    let after = read_capped(&t.path).unwrap();
    assert_eq!(frontmatter::body(&after), before);
    assert_eq!(store.project("p").unwrap().closed_count(), 1);
}

#[test]
fn links_dedupe() {
    let (_dir, store) = store();
    store.create_project(&slug("p"), "P", vec![]).unwrap();
    store.add_task("p", &slug("a"), "A", vec![]).unwrap();
    store.add_link("p", None, "ABC-9").unwrap();
    store.add_link("p", None, "ABC-9").unwrap();
    store.add_link("p", Some("1"), "backlog:TASK-3").unwrap();
    let p = store.project("p").unwrap();
    assert_eq!(p.meta.links, ["ABC-9"]);
    assert_eq!(p.tasks[0].meta.links, ["backlog:TASK-3"]);
}

#[test]
fn log_flattens_and_tags() {
    let (_dir, store) = store();
    store.create_project(&slug("p"), "P", vec![]).unwrap();
    store.add_task("p", &slug("a"), "A", vec![]).unwrap();
    store
        .append_log("p", Some("a"), "line one\nline two")
        .unwrap();
    let log = fs::read_to_string(store.project("p").unwrap().log_file()).unwrap();
    assert!(log
        .lines()
        .last()
        .unwrap()
        .ends_with("`01-a` line one line two"));
    assert!(matches!(
        store.append_log("p", Some("9"), "x"),
        Err(Error::TaskNotFound { .. })
    ));
}

#[test]
fn broken_files_become_warnings() {
    let (_dir, store) = store();
    store.create_project(&slug("good"), "Good", vec![]).unwrap();
    store.create_project(&slug("bad"), "Bad", vec![]).unwrap();
    let bad = store.project("bad").unwrap();
    fs::write(bad.file(), "no frontmatter").unwrap();
    let good = store.project("good").unwrap();
    fs::write(
        good.dir.join(TASKS_DIR).join("01-x.md"),
        "+++\ntitle = 1\n+++\n",
    )
    .unwrap();
    fs::write(good.dir.join(TASKS_DIR).join("notes.md"), "ignored").unwrap();
    let listing = store.list().unwrap();
    assert_eq!(listing.projects.len(), 1);
    assert_eq!(listing.warnings.len(), 2, "{:?}", listing.warnings);
}

#[test]
fn open_projects_sort_first() {
    let (_dir, store) = store();
    store.create_project(&slug("old"), "Old", vec![]).unwrap();
    store.create_project(&slug("new"), "New", vec![]).unwrap();
    store
        .set_project_status("new", ProjectStatus::Done)
        .unwrap();
    let ids: Vec<_> = store
        .list()
        .unwrap()
        .projects
        .into_iter()
        .map(|p| p.id.to_string())
        .collect();
    assert_eq!(ids, ["old", "new"]);
}

#[test]
fn handoff_prefers_working_then_latest_closed() {
    let (_dir, store) = store();
    store.create_project(&slug("p"), "P", vec![]).unwrap();
    for s in ["a", "b", "c"] {
        store.add_task("p", &slug(s), s, vec![]).unwrap();
    }
    let p = store.project("p").unwrap();
    assert!(p.handoff().is_none(), "placeholders are not next steps");
    let write_steps = |task: &Task, steps: &str| {
        let source = read_capped(&task.path).unwrap().replace(
                "<Filled in when the task closes or work pauses: what should happen next, and\nanything the next task (or the next session) needs to know to pick it up.>",
                steps,
            );
        fs::write(&task.path, source).unwrap();
    };
    write_steps(&p.tasks[0], "- open the PR");
    store.set_task_status("p", "1", TaskStatus::Done).unwrap();
    let p = store.project("p").unwrap();
    let (from, steps) = p.handoff().unwrap();
    assert_eq!(
        (from.id.as_str(), steps.as_str()),
        ("01-a", "- open the PR")
    );
    write_steps(&p.tasks[1], "- resume at step 2");
    store
        .set_task_status("p", "2", TaskStatus::Working)
        .unwrap();
    let p = store.project("p").unwrap();
    assert_eq!(p.handoff().unwrap().0.id, "02-b");
}

#[test]
fn placeholders() {
    assert!(is_placeholder("  "));
    assert!(is_placeholder("<fill\nme in>"));
    assert!(!is_placeholder("- done"));
}

#[test]
fn sessions_attach_once() {
    let (_dir, store) = store();
    store.create_project(&slug("p"), "P", vec![]).unwrap();
    store.add_task("p", &slug("a"), "A", vec![]).unwrap();
    assert!(store.attach_session("p", "1", "4f85a782-abc").unwrap());
    assert!(!store.attach_session("p", "1", "4f85a782-abc").unwrap());
    assert!(store.attach_session("p", "1", "../bad").is_err());
    let p = store.project("p").unwrap();
    assert_eq!(p.tasks[0].meta.sessions, ["4f85a782-abc"]);
    let log = fs::read_to_string(p.log_file()).unwrap();
    assert!(log.contains("`01-a` attached session 4f85a782"), "{log}");
    assert_eq!(store.attached_projects("4f85a782-abc"), ["p"]);
    store.add_task("p", &slug("b"), "B", vec![]).unwrap();
    store.attach_session("p", "2", "4f85a782-abc").unwrap();
    assert_eq!(
        store.attached_projects("4f85a782-abc"),
        ["p"],
        "listed once"
    );
    assert_eq!(store.attached_projects("unknown").len(), 0);
}

#[test]
fn quiet_working_tasks_pause_and_resume_on_activity() {
    use std::time::{Duration, SystemTime};
    let (_dir, store) = store();
    store.create_project(&slug("p"), "P", vec![]).unwrap();
    store.add_task("p", &slug("a"), "A", vec![]).unwrap();
    store.add_task("p", &slug("b"), "B", vec![]).unwrap();
    store
        .set_task_status("p", "1", TaskStatus::Working)
        .unwrap();
    let now = SystemTime::now();
    let long_ago = |_: &Project, _: &Task| Some(now - Duration::from_mins(40));
    let just_now = |_: &Project, _: &Task| Some(now);
    let none = store
        .pause_quiet_tasks(Duration::from_mins(30), just_now, now)
        .unwrap();
    assert_eq!(none.len(), 0, "active tasks stay working");
    let paused = store
        .pause_quiet_tasks(Duration::from_mins(30), long_ago, now)
        .unwrap();
    assert_eq!(paused.len(), 1);
    assert_eq!(paused[0].task, "01-a · A");
    assert_eq!(paused[0].quiet, "40m");
    let p = store.project("p").unwrap();
    assert_eq!(p.tasks[0].meta.status, TaskStatus::Paused);
    assert_eq!(
        p.tasks[1].meta.status,
        TaskStatus::Todo,
        "only working tasks pause"
    );
    let log = fs::read_to_string(p.log_file()).unwrap();
    assert!(log.contains("`01-a` auto-paused: quiet 40m"), "{log}");
    store.append_log("p", Some("1"), "back on it").unwrap();
    assert_eq!(
        store.project("p").unwrap().tasks[0].meta.status,
        TaskStatus::Working
    );
}

#[test]
fn task_stems() {
    assert_eq!(split_task_stem("01-a-b"), Some((1, "a-b")));
    assert_eq!(split_task_stem("120-x"), Some((120, "x")));
    assert_eq!(split_task_stem("x-1"), None);
    assert_eq!(split_task_stem("01-"), None);
    assert_eq!(split_task_stem("01"), None);
}

#[test]
fn goal_skips_placeholder() {
    let (_dir, store) = store();
    let p = store.create_project(&slug("p"), "P", vec![]).unwrap();
    assert_eq!(p.goal(), None);
    let source = read_capped(&p.file()).unwrap().replace(
        "<One or two sentences: what will be true when this project is finished.>",
        "Only our CDN can reach\nthe API.",
    );
    fs::write(p.file(), source).unwrap();
    assert_eq!(p.goal().as_deref(), Some("Only our CDN can reach the API."));
}
