//! Tests for the jobs and runs cursors.

use agent_core::types::{Run, RunState, Trigger};
use chrono::Utc;

use crate::app::App;

fn run(id: &str, state: RunState) -> Run {
    Run {
        id: id.to_owned(),
        project: "p".to_owned(),
        agent: "a".to_owned(),
        state,
        trigger: Trigger::Manual,
        parent_run_id: None,
        params: None,
        tmux_session: None,
        log_path: None,
        workdir: None,
        model: None,
        created_at: Utc::now(),
        started_at: None,
        finished_at: None,
        exit_code: None,
        cost_usd: None,
        num_turns: None,
        usage_json: None,
    }
}

#[test]
fn the_runs_cursor_moves_over_steps_and_never_stops_on_a_partition() {
    // Two runs of job `a`: root1 -> leaf1, and root2 alone. Three
    // steps, two partition headers. The cursor must only ever be on a
    // step, and must not run past the last one.
    let mut app = App::new();
    let mut root1 = run("root1", RunState::Succeeded);
    root1.created_at = Utc::now() - chrono::Duration::hours(2);
    let mut leaf1 = run("leaf1", RunState::Succeeded);
    leaf1.parent_run_id = Some("root1".to_owned());
    leaf1.agent = "step".to_owned();
    let root2 = run("root2", RunState::Succeeded);
    app.history = vec![root2, root1, leaf1];
    app.job_open = Some(("p".to_owned(), "a".to_owned()));

    assert_eq!(app.job_steps().len(), 3);
    let mut seen = Vec::new();
    for _ in 0..6 {
        seen.push(app.dag_selected_run().map(|r| r.id.clone()));
        app.dag_move(true);
    }
    assert_eq!(seen[0].as_deref(), Some("root2"), "newest run first");
    assert!(seen.iter().all(Option::is_some), "never on a header");
    assert_eq!(
        seen.last().unwrap().as_deref(),
        Some("leaf1"),
        "clamps at the last step"
    );
}

#[test]
fn with_nothing_declared_the_jobs_cursor_stays_put() {
    let mut app = App::new();
    app.dag_move(true);
    app.dag_move(false);
    assert_eq!(app.dag_selected, 0);
    assert!(app.selected_job().is_none());
}
