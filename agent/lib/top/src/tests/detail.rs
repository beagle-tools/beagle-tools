use agent_core::types::{RunState, Trigger};
use chrono::Utc;

use super::*;

fn run(params: Option<&str>) -> Run {
    Run {
        id: "r1".to_owned(),
        project: "work".to_owned(),
        agent: "ticket".to_owned(),
        state: RunState::Succeeded,
        trigger: Trigger::Manual,
        parent_run_id: None,
        params: params.map(str::to_owned),
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
fn the_ticket_param_is_read_from_the_run() {
    assert_eq!(
        ticket_param(&run(Some(r#"{"ticket":"ABC-123"}"#))).as_deref(),
        Some("ABC-123")
    );
    assert_eq!(ticket_param(&run(None)), None);
    assert_eq!(ticket_param(&run(Some("not json"))), None);
}

#[test]
fn a_ticket_links_through_the_configured_template() {
    // The ticket lives on the run row, so this one survives retention
    // even after the data dir holding the PR link is pruned.
    let ui = agent_core::ui::UiConfig {
        ticket_url: Some("https://tracker.example.com/issue/{key}".to_owned()),
        investigations_dir: None,
        hidden_projects: Vec::new(),
    };
    let links = run_links_with(&run(Some(r#"{"ticket":"ABC-123"}"#)), &ui);
    assert_eq!(links[0].label, "ticket ABC-123");
    assert_eq!(links[0].target, "https://tracker.example.com/issue/ABC-123");
    // No template: a bare key is not guessed into a broken link.
    let none = agent_core::ui::UiConfig::default();
    assert_eq!(
        run_links_with(&run(Some(r#"{"ticket":"ABC-123"}"#)), &none),
        [] as [Link; 0]
    );
    // No params, no data dir: nothing to offer rather than a broken link.
    assert_eq!(run_links_with(&run(None), &ui), [] as [Link; 0]);
}

#[test]
fn pr_urls_are_recovered_without_trailing_punctuation() {
    let dir = std::env::temp_dir().join(format!("agenttop-detail-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("result.md");
    std::fs::write(
        &f,
        "- **PR:** https://github.com/acme/widgets/pull/1333 (draft)\n",
    )
    .unwrap();
    assert_eq!(
        agent_core::links::first_url_in_file(&f, "/pull/").as_deref(),
        Some("https://github.com/acme/widgets/pull/1333")
    );
    assert_eq!(agent_core::links::first_url_in_file(&f, "/nope/"), None);
    let _ = std::fs::remove_dir_all(&dir);
}
