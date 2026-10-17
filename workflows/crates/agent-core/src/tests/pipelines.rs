use chrono::{TimeZone, Utc};

use super::*;
use crate::types::{ConcurrencyPolicy, Isolation, Runner, Trigger};

fn spec(name: &str, schedule: Option<&str>, entrypoint: Option<bool>) -> AgentSpec {
    AgentSpec {
        project: "p".to_owned(),
        name: name.to_owned(),
        source: std::path::PathBuf::new(),
        runner: Runner::Script,
        model: None,
        schedule: schedule.map(ToOwned::to_owned),
        workdir: "/tmp".into(),
        prompt: None,
        command: Some("true".to_owned()),
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
        entrypoint,
    }
}

fn run(id: &str, agent: &str, parent: Option<&str>, minute: u32, state: RunState) -> Run {
    Run {
        id: id.to_owned(),
        project: "p".to_owned(),
        agent: agent.to_owned(),
        state,
        trigger: Trigger::Manual,
        parent_run_id: parent.map(ToOwned::to_owned),
        params: None,
        tmux_session: None,
        log_path: None,
        workdir: None,
        model: None,
        created_at: Utc.with_ymd_and_hms(2026, 1, 1, 0, minute, 0).unwrap(),
        started_at: None,
        finished_at: None,
        exit_code: None,
        cost_usd: None,
        num_turns: None,
        usage_json: None,
    }
}

#[test]
fn an_agent_that_has_only_ever_been_triggered_is_a_step() {
    let runs = vec![
        run("r1", "root", None, 0, RunState::Succeeded),
        run("r2", "stage", Some("r1"), 1, RunState::Succeeded),
    ];
    assert!(is_entry(&spec("root", None, None), &runs));
    assert!(!is_entry(&spec("stage", None, None), &runs));
}

#[test]
fn a_scheduled_agent_is_an_entry_point_even_with_no_runs() {
    assert!(is_entry(&spec("nightly", Some("0 7 * * *"), None), &[]));
}

#[test]
fn an_agent_nobody_has_run_shows_up_rather_than_hiding() {
    assert!(is_entry(&spec("brand-new", None, None), &[]));
}

#[test]
fn the_manifest_can_say_so_outright() {
    let runs = vec![run("r1", "helper", None, 0, RunState::Succeeded)];
    // A root run would normally make it an entry point; the
    // declaration wins, so a step you sometimes run by hand can say
    // it is not a job.
    assert!(!is_entry(&spec("helper", None, Some(false)), &runs));
    assert!(is_entry(&spec("helper", None, Some(true)), &[]));
}

#[test]
fn a_pipeline_collects_its_steps_and_knows_where_it_is() {
    let specs = vec![spec("root", None, None), spec("stage", None, None)];
    let runs = vec![
        run("r1", "root", None, 0, RunState::Succeeded),
        run("r2", "stage", Some("r1"), 1, RunState::Running),
    ];
    let pipes = pipelines(&specs, &runs);
    assert_eq!(pipes.len(), 1, "stage must not be its own pipeline");
    assert_eq!(pipes[0].entry.name, "root");
    assert_eq!(pipes[0].steps, vec!["stage".to_owned()]);
    assert!(pipes[0].is_running());
    assert_eq!(pipes[0].active_steps(), vec!["stage"]);
}

#[test]
fn steps_read_in_the_order_they_run() {
    // root -> a -> b, and root -> c. Observed order here is b, c, a;
    // the sequence people read must still be a, then b and c.
    let specs = vec![
        spec("root", None, None),
        spec("a", None, None),
        spec("b", None, None),
        spec("c", None, None),
    ];
    let runs = vec![
        run("r1", "root", None, 0, RunState::Succeeded),
        run("r3", "b", Some("r2"), 2, RunState::Succeeded),
        run("r4", "c", Some("r1"), 3, RunState::Succeeded),
        run("r2", "a", Some("r1"), 1, RunState::Succeeded),
    ];
    let pipes = pipelines(&specs, &runs);
    assert_eq!(
        pipes[0].steps,
        vec!["a".to_owned(), "c".to_owned(), "b".to_owned()]
    );
}

#[test]
fn two_chains_at_different_steps_report_both() {
    let specs = vec![
        spec("root", None, None),
        spec("a", None, None),
        spec("b", None, None),
    ];
    let runs = vec![
        run("r1", "root", None, 0, RunState::Succeeded),
        run("r2", "a", Some("r1"), 1, RunState::Running),
        run("r3", "root", None, 2, RunState::Succeeded),
        run("r4", "b", Some("r3"), 3, RunState::Running),
    ];
    let pipes = pipelines(&specs, &runs);
    assert_eq!(pipes.len(), 1);
    assert_eq!(pipes[0].active_steps(), vec!["a", "b"]);
}

#[test]
fn display_order_nests_steps_under_their_job_and_loses_nobody() {
    let specs = vec![
        spec("root", None, None),
        spec("stage", None, None),
        spec("orphan-step", None, Some(false)),
    ];
    let runs = vec![
        run("r1", "root", None, 0, RunState::Succeeded),
        run("r2", "stage", Some("r1"), 1, RunState::Succeeded),
    ];
    let order: Vec<String> = display_order(&specs, &runs)
        .into_iter()
        .map(|s| s.name)
        .collect();
    // Job first, its step under it, and the step nothing claims
    // still present rather than dropped.
    assert_eq!(order, vec!["root", "stage", "orphan-step"]);
}

#[test]
fn running_pipelines_sort_above_idle_ones() {
    let specs = vec![spec("busy", None, None), spec("idle", None, None)];
    let runs = vec![
        // idle ran more recently, but busy is live.
        run("r1", "busy", None, 0, RunState::Running),
        run("r2", "idle", None, 9, RunState::Succeeded),
    ];
    let pipes = pipelines(&specs, &runs);
    assert_eq!(pipes[0].entry.name, "busy");
}
