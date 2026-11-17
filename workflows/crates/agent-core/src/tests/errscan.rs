use std::path::Path;

use chrono::Duration;

use super::*;
use crate::types::Trigger;

fn temp_store(tag: &str) -> (Store, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("agent-core-errscan-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("data")).unwrap();
    let store = Store::open(&dir.join("state.db")).unwrap();
    (store, dir)
}

/// A finished run of `agent`, under `parent`, that logged `lines`.
fn run(store: &Store, agent: &str, parent: Option<&str>, state: RunState, lines: &[&str]) -> Run {
    let run = store
        .create_pending_run("p", agent, Trigger::Manual, parent, None, None)
        .unwrap();
    store
        .mark_running(&run.id, "s", Path::new("/nonexistent.log"), Path::new("/"))
        .unwrap();
    for line in lines {
        store.insert_event(&run.id, "raw", Some(line)).unwrap();
    }
    store.finish_run(&run.id, state, Some(1)).unwrap();
    run
}

fn window() -> (DateTime<Utc>, DateTime<Utc>) {
    (
        Utc::now() - Duration::hours(1),
        Utc::now() + Duration::minutes(1),
    )
}

#[test]
fn the_same_failure_in_the_same_step_is_one_group_with_a_count() {
    let (store, dir) = temp_store("dedupe");
    let gate = run(&store, "gate", None, RunState::Succeeded, &[]);
    for n in [3, 7, 12] {
        run(
            &store,
            "step",
            Some(&gate.id),
            RunState::Failed,
            &[&format!(
                "ERROR step: PR #{n} failed after {n}s (run step-20261027{n:06}-ab12cd34)"
            )],
        );
    }
    run(
        &store,
        "step",
        Some(&gate.id),
        RunState::Failed,
        &["ERROR step: something else"],
    );

    let (since, now) = window();
    let groups = scan(&store, since, now, &dir.join("data")).unwrap();
    assert_eq!(groups.len(), 2);
    let repeated = groups.iter().find(|g| g.count == 3).unwrap();
    assert_eq!(repeated.step, "p/step");
    assert_eq!(repeated.job, "p/gate");
    assert_eq!(repeated.kind, ErrorKind::Failed);
    assert_eq!(repeated.runs.len(), 3);

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_error_line_in_a_run_that_carried_on_still_counts() {
    let (store, dir) = temp_store("logged");
    run(
        &store,
        "gate",
        None,
        RunState::Succeeded,
        &[
            "INFO gate: fine",
            "ERROR gate: cluster-a: cannot authenticate",
            "ERROR gate: cluster-b: cannot authenticate",
            "OK gate: done",
        ],
    );
    let (since, now) = window();
    let groups = scan(&store, since, now, &dir.join("data")).unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].kind, ErrorKind::Logged);
    assert_eq!(
        groups[0].message,
        "gate: cluster-a: cannot authenticate | gate: cluster-b: cannot authenticate"
    );

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn killed_runs_and_runs_outside_the_window_are_not_errors() {
    let (store, dir) = temp_store("skip");
    run(&store, "a", None, RunState::Killed, &[]);
    run(&store, "b", None, RunState::Failed, &["ERROR b: old"]);
    // A window that ended before these runs finished.
    let groups = scan(
        &store,
        Utc::now() - Duration::hours(3),
        Utc::now() - Duration::hours(2),
        &dir.join("data"),
    )
    .unwrap();
    assert!(groups.is_empty());
    let (since, now) = window();
    let groups = scan(&store, since, now, &dir.join("data")).unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].step, "p/b");

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_failure_with_no_error_line_falls_back_to_the_runner_result_then_the_exit() {
    let (store, dir) = temp_store("reason");
    let r = run(&store, "claude-step", None, RunState::Failed, &[]);
    store
        .insert_event(
            &r.id,
            "result",
            Some(r#"{"is_error":true,"result":"permission denied: Write"}"#),
        )
        .unwrap();
    run(&store, "quiet", None, RunState::TimedOut, &[]);

    let (since, now) = window();
    let groups = scan(&store, since, now, &dir.join("data")).unwrap();
    let claude = groups.iter().find(|g| g.step == "p/claude-step").unwrap();
    assert_eq!(claude.message, "permission denied: Write");
    let quiet = groups.iter().find(|g| g.step == "p/quiet").unwrap();
    assert_eq!(quiet.kind, ErrorKind::TimedOut);
    assert!(quiet.message.starts_with("timed out"));

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn signatures_mask_what_changes_between_occurrences() {
    assert_eq!(
        signature("pr-check-20261026185858-6aae724e: PR #2307 took 12.5s"),
        signature("pr-check-20261027101010-ffffffff: PR #806 took 3.0s")
    );
    assert_ne!(
        signature("cannot authenticate"),
        signature("cannot find context")
    );
    assert_eq!(signature("waited 30s, then 600s"), "waited <id>, then <id>");
}
