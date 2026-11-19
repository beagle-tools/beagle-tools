// Tests panic on a broken fixture; that is the failure mode we want.
#![allow(clippy::unwrap_used)]

use chrono::Utc;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tandem_cache::{Cache, ReviewQueue, ReviewStore, ReviewTriage};
use tandem_core::agent::{AgentRun, RunStatus};
use tandem_core::diff_parse::parse_unified_diff;
use tandem_core::github::RepoRef;
use tandem_core::review::CommentStatus;

use super::keys::Flow;
use super::*;
use crate::state::Stage;

const DIFF: &str =
    "diff --git a/a.rs b/a.rs\n--- a/a.rs\n+++ b/a.rs\n@@ -1,2 +1,2 @@\n keep\n-old\n+new\n";

/// One PR reviewed (two comments) and one still queued.
fn app() -> App {
    let cache = Cache::open_in_memory().unwrap();
    let repo = RepoRef::parse("o/r").unwrap();
    crate::jobs::add(&cache, &PrRef::parse("o/r#1").unwrap(), "test").unwrap();
    let job = cache.claim_review_job().unwrap().unwrap();
    cache
        .put_diff(&repo, 1, "sha", DIFF, &parse_unified_diff(DIFF).unwrap())
        .unwrap();
    cache
        .put_agent_run(&AgentRun {
            run_id: "r1".into(),
            agent_name: "claude".into(),
            repo_slug: "o/r".into(),
            pr_number: 1,
            head_sha: "sha".into(),
            status: RunStatus::Running,
            started_at: Utc::now(),
            finished_at: None,
            log_path: String::new(),
            comment_count: 0,
            purpose: "pr review".into(),
            target_comment_id: None,
            error: None,
        })
        .unwrap();
    cache.finish_review_job(&job.id, Some("r1"), None).unwrap();
    let review = [
        r#"{"type":"tandem_verdict","verdict":"not-mergeable","reason":"bug"}"#,
        r###"{"type":"tandem_summary","body":"## 1. What is the problem\nX"}"###,
        r#"{"type":"tandem_comment","path":"a.rs","line":2,"severity":"issue","body":"off by one","suggestion":"fixed","context":"why it matters"}"#,
        r#"{"type":"tandem_comment","path":"","line":0,"severity":"info","tag":"duplicate","body":"agree","context":"c","duplicate_of":"@bot x"}"#,
    ]
    .join("\n");
    crate::jobs::ingest(&cache, "r1", &review).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(5));
    cache.enqueue_review("o/r", 2, None, "test").unwrap();
    let mut app = App::new(cache, Config::default());
    app.reload().unwrap();
    app
}

fn press(app: &mut App, code: KeyCode) -> Flow {
    app.handle_key(KeyEvent::new(code, KeyModifiers::NONE))
}

fn draw(app: &mut App) -> String {
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
    terminal.draw(|f| view::draw(f, app)).unwrap();
    let buffer = terminal.backend().buffer().clone();
    buffer
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect()
}

#[test]
fn the_list_puts_ready_first_and_draws_every_tab() {
    let mut app = app();
    assert_eq!(app.rows.len(), 2);
    assert_eq!(app.rows[0].stage, Stage::Ready);
    assert_eq!(app.rows[1].stage, Stage::Queued);
    let screen = draw(&mut app);
    assert!(screen.contains("o/r#1"), "{screen}");
    assert!(screen.contains("Not mergeable, there are issues"));
    for tab in ['2', '3'] {
        press(&mut app, KeyCode::Char(tab));
        assert_ne!(draw(&mut app), "");
    }
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Char('j'));
    assert!(draw(&mut app).contains("history (local, never posted)"));
}

#[test]
fn navigation_and_tabs() {
    let mut app = app();
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(app.sel, 1);
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(app.sel, 1, "clamped");
    press(&mut app, KeyCode::Char('g'));
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.focus, Focus::Content);
    press(&mut app, KeyCode::Tab);
    assert_eq!(app.tab, Tab::Diff);
    press(&mut app, KeyCode::BackTab);
    assert_eq!(app.tab, Tab::Summary);
    press(&mut app, KeyCode::Esc);
    assert_eq!(app.focus, Focus::List);
    press(&mut app, KeyCode::Char('?'));
    assert_eq!(app.mode, Mode::Help);
    press(&mut app, KeyCode::Char('z'));
    assert_eq!(app.mode, Mode::Normal);
    assert_eq!(press(&mut app, KeyCode::Char('q')), Flow::Quit);
}

#[test]
fn comments_select_dismiss_and_ask_before_posting() {
    let mut app = app();
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Char('3'));
    // PR-level comment sorts first, then a.rs:2.
    assert_eq!(app.comment().unwrap().path, "");
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(app.comment().unwrap().path, "a.rs");

    press(&mut app, KeyCode::Char('x'));
    assert_eq!(app.comment().unwrap().status, CommentStatus::Rejected);
    press(&mut app, KeyCode::Char('x'));
    assert_eq!(app.comment().unwrap().status, CommentStatus::Open);

    press(&mut app, KeyCode::Char('p'));
    let Mode::Confirm(Action::Post { preview, .. }) = &app.mode else {
        unreachable!("expected a post confirmation, got {:?}", app.mode);
    };
    assert!(preview.contains("```suggestion\nfixed\n```"), "{preview}");
    assert!(draw(&mut app).contains("Post this comment to GitHub"));
    press(&mut app, KeyCode::Char('n'));
    assert_eq!(app.mode, Mode::Normal);
    assert!(app.comment().unwrap().posted_github_id.is_none());

    press(&mut app, KeyCode::Enter);
    assert_eq!(app.tab, Tab::Diff, "Enter shows the comment's file");
}

#[test]
fn approve_asks_and_marks_toggle() {
    let mut app = app();
    press(&mut app, KeyCode::Char('A'));
    assert!(matches!(app.mode, Mode::Confirm(Action::Approve(_))));
    press(&mut app, KeyCode::Esc);

    press(&mut app, KeyCode::Char('m'));
    let done = app.rows.iter().find(|r| r.pr.number == 1).unwrap();
    assert_eq!(done.stage, Stage::Done);
    assert_eq!(
        app.selected().unwrap().pr.number,
        1,
        "selection follows the PR"
    );
    press(&mut app, KeyCode::Char('m'));
    assert_eq!(app.selected().unwrap().stage, Stage::Ready);

    // A queued PR can't be approved yet.
    press(&mut app, KeyCode::Char('G'));
    press(&mut app, KeyCode::Char('A'));
    assert_eq!(app.mode, Mode::Normal);
    assert!(app.status.as_ref().unwrap().1, "an error is shown");
}

#[test]
fn queueing_from_the_input_line() {
    let mut app = app();
    press(&mut app, KeyCode::Char('a'));
    for c in "nope".chars() {
        press(&mut app, KeyCode::Char(c));
    }
    press(&mut app, KeyCode::Enter);
    assert!(app.status.as_ref().unwrap().0.contains("not a PR"));

    press(&mut app, KeyCode::Char('a'));
    for c in "https://github.com/o/r/pull/9".chars() {
        press(&mut app, KeyCode::Char(c));
    }
    press(&mut app, KeyCode::Backspace);
    press(&mut app, KeyCode::Char('9'));
    press(&mut app, KeyCode::Enter);
    assert!(app
        .rows
        .iter()
        .any(|r| r.pr.number == 9 && r.stage == Stage::Queued));
}

#[test]
fn w_resizes_the_list_within_its_limits_and_esc_leaves() {
    let mut a = app();
    draw(&mut a); // records the screen width
    let start = super::resize::list_width(a.list_width, a.main_width);
    press(&mut a, KeyCode::Char('w'));
    assert_eq!(a.mode, Mode::Resize);
    press(&mut a, KeyCode::Right);
    assert_eq!(a.list_width, Some(start + super::resize::STEP));
    // q leaves resize mode rather than quitting.
    press(&mut a, KeyCode::Char('q'));
    assert_eq!(a.mode, Mode::Normal);
    press(&mut a, KeyCode::Char('w'));
    for _ in 0..200 {
        press(&mut a, KeyCode::Left);
    }
    assert_eq!(a.list_width, Some(super::resize::MIN_LIST));
    assert!(a
        .status
        .as_ref()
        .is_some_and(|(m, _)| m.contains("narrowest")));
    for _ in 0..200 {
        press(&mut a, KeyCode::Char('l'));
    }
    let widest = a.list_width.unwrap();
    assert!(a.main_width - widest >= super::resize::MIN_CONTENT);
    press(&mut a, KeyCode::Esc);
    assert_eq!(a.mode, Mode::Normal);
    // The choice survives a redraw.
    draw(&mut a);
    assert_eq!(a.list_width, Some(widest));
}
