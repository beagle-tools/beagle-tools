use std::path::PathBuf;

use agent_core::types::{RunState, Trigger};

use super::*;

/// A test's own summaries/ and data/ under its temp dir — never the
/// real ones. See `Dirs` for what happened when a pass found them
/// itself.
fn test_dirs(dir: &Path) -> Dirs {
    let dirs = Dirs {
        summaries: dir.join("summaries"),
        data: dir.join("data"),
    };
    std::fs::create_dir_all(&dirs.summaries).unwrap();
    std::fs::create_dir_all(&dirs.data).unwrap();
    dirs
}

#[test]
fn a_pass_only_touches_the_directories_it_is_given() {
    // The regression: a pass against a throwaway store deleted every
    // write-up in the REAL summaries dir, because none of the real runs
    // were in the throwaway store. Stand a second dir in for "real":
    // the pass is handed `mine`, and `elsewhere` must be untouched.
    let dir = temp_dir("only-its-dirs");
    let store = Store::open(&dir.join("state.db")).unwrap();
    let mine = test_dirs(&dir.join("mine"));
    let elsewhere = test_dirs(&dir.join("elsewhere"));
    let orphan = "ticket-20260101000000-0rphan00.md";
    std::fs::write(mine.summaries.join(orphan), "x").unwrap();
    std::fs::write(elsewhere.summaries.join(orphan), "x").unwrap();

    tick(&store, &[], Utc::now(), 4, &mine).unwrap();

    assert!(
        !mine.summaries.join(orphan).exists(),
        "its own orphan is swept"
    );
    assert!(
        elsewhere.summaries.join(orphan).exists(),
        "a write-up outside the dirs it was handed must never be touched"
    );
    let _ = std::fs::remove_dir_all(dir);
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("agentd-retention-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn cutoff_disabled_at_zero_hours() {
    let now = Utc::now();
    assert!(cutoff(now, 0).is_none());
    assert_eq!(cutoff(now, 4).unwrap(), now - Duration::hours(4));
}

#[test]
fn prunes_old_run_logs_but_spares_recent_ones() {
    let dir = temp_dir("prune");
    let store = Store::open(&dir.join("state.db")).unwrap();
    let log = dir.join("old.jsonl");
    std::fs::write(&log, "{}\n").unwrap();

    let run = store
        .create_pending_run("p", "a", Trigger::Manual, None, None, None)
        .unwrap();
    store.mark_running(&run.id, "agentd-x", &log, &dir).unwrap();
    store.insert_event(&run.id, "message", Some("hi")).unwrap();
    store
        .finish_run(&run.id, RunState::Succeeded, Some(0))
        .unwrap();

    // Within the window: nothing happens.
    tick(&store, &[], Utc::now(), 4, &test_dirs(&dir)).unwrap();
    assert!(log.is_file());
    assert_eq!(store.events_after(&run.id, 0, 10).unwrap().len(), 1);

    // Window elapsed (simulated by a future clock): log + events go.
    tick(
        &store,
        &[],
        Utc::now() + Duration::hours(5),
        4,
        &test_dirs(&dir),
    )
    .unwrap();
    assert!(!log.exists());
    assert_eq!(
        store.events_after(&run.id, 0, 10).unwrap(),
        [] as [agent_core::types::Event; 0]
    );
    assert!(store.get_run(&run.id).unwrap().is_some());

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_data_dir_outlives_its_log_by_the_resume_window() {
    let dir = temp_dir("data-window");
    let store = Store::open(&dir.join("state.db")).unwrap();
    let dirs = test_dirs(&dir);
    let run = store
        .create_pending_run("p", "a", Trigger::Manual, None, None, None)
        .unwrap();
    store
        .finish_run(&run.id, RunState::Failed, Some(1))
        .unwrap();
    let live = store
        .create_pending_run("p", "a", Trigger::Manual, None, None, None)
        .unwrap();
    for id in [&run.id, &live.id] {
        std::fs::create_dir_all(dirs.data.join(id)).unwrap();
    }

    // Past the 4h log window: the handover is still there to resume from.
    tick(&store, &[], Utc::now() + Duration::hours(5), 4, &dirs).unwrap();
    assert!(dirs.data.join(&run.id).is_dir());

    // Past the week: gone. The still-pending run keeps its dir.
    let later = Utc::now() + Duration::hours(agent_core::resume::DATA_RETENTION_HOURS + 1);
    tick(&store, &[], later, 4, &dirs).unwrap();
    assert!(!dirs.data.join(&run.id).exists());
    assert!(dirs.data.join(&live.id).is_dir());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn noop_runs_die_young_but_the_newest_survives() {
    let dir = temp_dir("noop");
    let store = Store::open(&dir.join("state.db")).unwrap();
    let spec = AgentSpec {
        project: "demo".to_owned(),
        name: "watch-poll".to_owned(),
        source: PathBuf::new(),
        purpose: None,
        runner: agent_core::types::Runner::Script,
        model: None,
        schedule: None,
        timezone: agent_core::types::ScheduleTz::Utc,
        workdir: "/tmp".into(),
        prompt: None,
        command: Some("watch-poll".to_owned()),
        allowed_tools: None,
        disallowed_tools: None,
        append_system_prompt: None,
        add_dirs: Vec::new(),
        params: Vec::new(),
        requires_mcp: Vec::new(),
        mcp_config: None,
        env: std::collections::BTreeMap::default(),
        env_file: None,
        database: false,
        max_concurrent: None,
        noop_retention_minutes: Some(1),
        timeout_minutes: 2,
        notify: None,
        notify_icon: None,
        notify_icon_failure: None,
        concurrency: agent_core::types::ConcurrencyPolicy::Forbid,
        isolation: agent_core::types::Isolation::None,
        sandbox: agent_core::types::CodexSandbox::Off,
        enabled: true,
        entrypoint: None,
        triggers: Vec::new(),
        allow_github_writes: false,
    };

    let mut ids = Vec::new();
    for _ in 0..3 {
        let run = store
            .create_pending_run("demo", "watch-poll", Trigger::Cron, None, None, None)
            .unwrap();
        store
            .mark_running(&run.id, "s", &dir.join("x.jsonl"), &dir)
            .unwrap();
        store
            .finish_run(&run.id, RunState::Succeeded, Some(0))
            .unwrap();
        ids.push(run.id);
    }
    // A timed-out poll (slept-through) is a no-op too; a failed run must
    // never be touched by the no-op pass.
    let slept = store
        .create_pending_run("demo", "watch-poll", Trigger::Cron, None, None, None)
        .unwrap();
    store
        .finish_run(&slept.id, RunState::TimedOut, None)
        .unwrap();
    let failed = store
        .create_pending_run("demo", "watch-poll", Trigger::Cron, None, None, None)
        .unwrap();
    store
        .finish_run(&failed.id, RunState::Failed, Some(1))
        .unwrap();

    // Not old enough yet: everything stays.
    tick(
        &store,
        std::slice::from_ref(&spec),
        Utc::now(),
        4,
        &test_dirs(&dir),
    )
    .unwrap();
    assert_eq!(store.list_runs(&RunFilter::default()).unwrap().len(), 5);

    // Two minutes later: old no-ops (succeeded AND timed-out) go; the
    // newest of them + the failed run stay.
    tick(
        &store,
        std::slice::from_ref(&spec),
        Utc::now() + Duration::minutes(2),
        4,
        &test_dirs(&dir),
    )
    .unwrap();
    let left = store.list_runs(&RunFilter::default()).unwrap();
    assert_eq!(left.len(), 2);
    assert!(left.iter().any(|r| r.id == failed.id));
    // The newest prunable (the timed-out one, created last) is the
    // keeper; all three older succeeded no-ops are gone.
    assert!(left.iter().any(|r| r.id == slept.id));
    assert!(ids.iter().all(|id| !left.iter().any(|r| &r.id == id)));

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn missing_log_file_still_prunes_events() {
    let dir = temp_dir("missing");
    let store = Store::open(&dir.join("state.db")).unwrap();
    let run = store
        .create_pending_run("p", "a", Trigger::Manual, None, None, None)
        .unwrap();
    store
        .mark_running(&run.id, "agentd-x", &dir.join("never-created.jsonl"), &dir)
        .unwrap();
    store
        .finish_run(&run.id, RunState::Failed, Some(1))
        .unwrap();

    tick(
        &store,
        &[],
        Utc::now() + Duration::hours(5),
        4,
        &test_dirs(&dir),
    )
    .unwrap();
    // Only the kept evidence (here, state changes) is left.
    assert!(store
        .events_after(&run.id, 0, 10)
        .unwrap()
        .iter()
        .all(|e| e.kind == "state"));

    let _ = std::fs::remove_dir_all(dir);
}
