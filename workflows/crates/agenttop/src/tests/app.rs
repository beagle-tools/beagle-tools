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

fn app_with_rows() -> App {
    let mut app = App::new();
    app.active = vec![run("r1", RunState::Running), run("r2", RunState::Pending)];
    app.recent = vec![run("r3", RunState::Succeeded)];
    app
}

#[test]
fn selection_spans_active_then_recent() {
    let mut app = app_with_rows();
    assert_eq!(app.row_count(), 3);
    assert_eq!(app.selected_run().unwrap().id, "r1");
    app.move_selection(true);
    app.move_selection(true);
    assert_eq!(app.selected_run().unwrap().id, "r3");
    app.move_selection(true); // clamped at the end
    assert_eq!(app.selected_run().unwrap().id, "r3");
    app.move_selection(false);
    assert_eq!(app.selected_run().unwrap().id, "r2");
}

#[test]
fn selection_clamps_when_rows_shrink() {
    let mut app = app_with_rows();
    app.selected = 2;
    app.recent.clear();
    app.selected = clamp_selection(app.selected, app.row_count());
    assert_eq!(app.selected_run().unwrap().id, "r2");
}

#[test]
fn empty_app_has_no_selection() {
    let mut app = App::new();
    assert!(app.is_empty());
    assert!(app.selected_run().is_none());
    app.move_selection(true);
    assert_eq!(app.selected, 0);
}

#[test]
fn attach_target_requires_running_with_session() {
    let mut app = app_with_rows();
    assert_eq!(app.attach_target().unwrap(), "agentd-r1");
    app.move_selection(true); // pending
    assert!(app.attach_target().is_none());
    app.active[0].tmux_session = None;
    app.selected = 0;
    assert!(app.attach_target().is_none());
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

#[test]
fn activity_keeps_the_newest_event_per_run() {
    let mk = |id: i64, run_id: &str, kind: &str, payload: &str| Event {
        id,
        run_id: run_id.to_owned(),
        ts: Utc::now(),
        kind: kind.to_owned(),
        payload: Some(payload.to_owned()),
    };
    let events = vec![
        mk(1, "r1", "state", "running"),
        mk(2, "r1", "tool_use", r#"{"name":"Bash"}"#),
        mk(3, "r2", "message", "hi"),
    ];
    let map = build_activity(&events);
    assert_eq!(map["r1"], "tool_use Bash");
    assert_eq!(map["r2"], "message hi");
}
