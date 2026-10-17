use super::*;
use crate::types::{ConcurrencyPolicy, Isolation, Runner};

fn mem_store() -> Store {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(crate::schema::SCHEMA).unwrap();
    Store { conn }
}

fn spec(project: &str, name: &str) -> AgentSpec {
    AgentSpec {
        project: project.to_owned(),
        name: name.to_owned(),
        source: std::path::PathBuf::new(),
        runner: Runner::Claude,
        model: None,
        schedule: Some("0 7 * * *".to_owned()),
        workdir: "/tmp".into(),
        prompt: Some("/tmp/prompt.md".into()),
        command: None,
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
        noop_retention_minutes: None,
        notify: None,
        notify_icon: None,
        notify_icon_failure: None,
        timeout_minutes: 30,
        concurrency: ConcurrencyPolicy::Forbid,
        isolation: Isolation::None,
        enabled: true,
        entrypoint: None,
    }
}

#[test]
fn reconcile_replaces_agent_set() {
    let mut store = mem_store();
    store
        .reconcile_agents(&[spec("personal", "digest"), spec("personal", "prs")])
        .unwrap();
    assert_eq!(store.list_agents().unwrap().len(), 2);
    store
        .reconcile_agents(&[spec("personal", "digest")])
        .unwrap();
    let agents = store.list_agents().unwrap();
    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0].qualified_name(), "personal/digest");
    assert!(store.get_agent("personal", "prs").unwrap().is_none());
}

#[test]
fn run_lifecycle_pending_running_finished() {
    let store = mem_store();
    let run = store
        .create_pending_run("personal", "digest", Trigger::Manual, None, None, None)
        .unwrap();
    assert_eq!(store.pending_runs().unwrap().len(), 1);
    assert_eq!(store.active_run_count("personal", "digest").unwrap(), 1);

    store
        .mark_running(
            &run.id,
            "agentd-x",
            Path::new("/tmp/x.jsonl"),
            Path::new("/tmp"),
        )
        .unwrap();
    assert_eq!(store.pending_runs().unwrap().len(), 0);
    assert_eq!(store.running_runs().unwrap().len(), 1);

    store
        .finish_run(&run.id, RunState::Succeeded, Some(0))
        .unwrap();
    let done = store.get_run(&run.id).unwrap().unwrap();
    assert_eq!(done.state, RunState::Succeeded);
    assert_eq!(done.exit_code, Some(0));
    assert!(done.finished_at.is_some());
    assert_eq!(store.active_run_count("personal", "digest").unwrap(), 0);
}

#[test]
fn events_tail_incrementally() {
    let store = mem_store();
    let run = store
        .create_pending_run("p", "a", Trigger::Cron, None, None, None)
        .unwrap();
    store
        .insert_event(&run.id, "state", Some("pending"))
        .unwrap();
    store
        .insert_event(&run.id, "tool_use", Some("Bash"))
        .unwrap();
    let first = store.events_after(&run.id, 0, 10).unwrap();
    assert_eq!(first.len(), 2);
    let rest = store.events_after(&run.id, first[1].id, 10).unwrap();
    assert_eq!(rest, [] as [crate::types::Event; 0]);
    assert_eq!(store.recent_events(1).unwrap()[0].kind, "tool_use");
}

#[test]
fn run_filter_by_state_and_limit() {
    let store = mem_store();
    let a = store
        .create_pending_run("p", "a", Trigger::Cron, None, None, None)
        .unwrap();
    let _b = store
        .create_pending_run("p", "b", Trigger::Cron, None, None, None)
        .unwrap();
    store.finish_run(&a.id, RunState::Failed, Some(1)).unwrap();
    let failed = store
        .list_runs(&RunFilter {
            states: Some(vec![RunState::Failed]),
            ..RunFilter::default()
        })
        .unwrap();
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].id, a.id);
    let meta_roundtrip = store.get_meta("last_tick").unwrap();
    assert!(meta_roundtrip.is_none());
    store.set_meta("last_tick", "2026-01-01T00:00:00Z").unwrap();
    assert_eq!(
        store.get_meta("last_tick").unwrap().unwrap(),
        "2026-01-01T00:00:00Z"
    );
}

#[test]
fn run_model_override_round_trips() {
    let store = mem_store();
    let created = store
        .create_pending_run("p", "a", Trigger::Manual, None, None, Some("opus"))
        .unwrap();
    assert_eq!(created.model.as_deref(), Some("opus"));
    let loaded = store.get_run(&created.id).unwrap().unwrap();
    assert_eq!(loaded.model.as_deref(), Some("opus"));

    let plain = store
        .create_pending_run("p", "b", Trigger::Manual, None, None, None)
        .unwrap();
    assert_eq!(store.get_run(&plain.id).unwrap().unwrap().model, None);
}
