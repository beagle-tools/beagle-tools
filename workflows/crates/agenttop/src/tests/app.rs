use super::*;
use agent_core::types::Trigger;
use chrono::Utc;

fn run(id: &str, state: RunState) -> Run {
    Run {
        id: id.to_owned(),
        project: "p".to_owned(),
        agent: "a".to_owned(),
        state,
        trigger: Trigger::Manual,
        parent_run_id: None,
        params: None,
        tmux_session: Some(format!("agentd-{id}")),
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
fn attach_target_is_the_selected_step_only_while_it_runs() {
    // x and t act on the step under the runs-screen cursor.
    let mut app = App::new();
    let mut live = run("r1", RunState::Running);
    live.created_at = Utc::now();
    let mut done = run("r0", RunState::Succeeded);
    done.created_at = Utc::now() - chrono::Duration::hours(1);
    app.history = vec![live, done];
    app.job_open = Some(("p".to_owned(), "a".to_owned()));

    assert_eq!(app.attach_target().unwrap(), "agentd-r1");
    app.dag_move(true); // the finished run
    assert!(app.attach_target().is_none());
}

#[test]
fn all_runs_shows_every_jobs_chains_and_r_keeps_the_live_ones() {
    let mut app = App::new();
    let mut live = run("j1", RunState::Running);
    live.agent = "job-one".to_owned();
    let mut child = run("j1-step", RunState::Pending);
    child.agent = "step".to_owned();
    child.parent_run_id = Some("j1".to_owned());
    let mut done = run("j2", RunState::Succeeded);
    done.agent = "job-two".to_owned();
    done.created_at = Utc::now() - chrono::Duration::hours(1);
    app.history = vec![live, child, done];

    app.open_all_runs();
    assert!(app.runs_open());
    let steps: Vec<&str> = app.job_steps().iter().map(|r| r.id.as_str()).collect();
    assert_eq!(steps, vec!["j1", "j1-step", "j2"]);
    // R on a step of another job's chain targets that chain's job.
    app.dag_move(true);
    assert_eq!(
        app.root_of(app.dag_selected_run().unwrap()).agent,
        "job-one"
    );

    app.all_running_only = true;
    let steps: Vec<&str> = app.job_steps().iter().map(|r| r.id.as_str()).collect();
    assert_eq!(steps, vec!["j1", "j1-step"]);

    // W rides the top; moving stops it.
    app.toggle_follow();
    assert!(app.follow_top);
    assert_eq!(app.job_step_selected, 0);
    app.dag_move(true);
    assert!(!app.follow_top);

    app.close_job();
    assert!(!app.runs_open());
}

#[test]
fn log_view_scroll_clamps_to_event_count() {
    let mut app = App::new();
    app.log_view = Some(LogView {
        run: run("r1", RunState::Succeeded),
        events: (0..3)
            .map(|i| Event {
                id: i,
                run_id: "r1".to_owned(),
                ts: Utc::now(),
                kind: "message".to_owned(),
                payload: None,
            })
            .collect(),
        scroll_from_bottom: 0,
        scroll_x: 0,
        search: None,
        search_input: false,
        search_at: None,
        search_line: None,
    });
    for _ in 0..10 {
        app.scroll_logs(true, 2);
    }
    assert_eq!(app.log_view.as_ref().unwrap().scroll_from_bottom, 2);
    app.scroll_logs(false, 2);
    assert_eq!(app.log_view.as_ref().unwrap().scroll_from_bottom, 1);
    app.pan_logs(true);
    assert_eq!(app.log_view.as_ref().unwrap().scroll_x, 8);
    app.pan_logs(false);
    app.pan_logs(false); // clamped at 0
    assert_eq!(app.log_view.as_ref().unwrap().scroll_x, 0);
    app.close_logs();
    assert!(app.log_view.is_none());
}
