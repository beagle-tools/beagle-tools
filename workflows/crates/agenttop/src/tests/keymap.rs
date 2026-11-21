//! agenttop's keys: the standard keymap walk (`beagle_tui::keymap`), the
//! keys that moved to make room for it, and the layers keys pass through.

use agent_core::types::{
    AgentSpec, CodexSandbox, ConcurrencyPolicy, Isolation, Run, RunState, Runner, ScheduleTz,
    Trigger,
};
use chrono::Utc;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{App, LogView};
use crate::input::{self, Effect};
use crate::keys::{self, Action};

/// A small frame: ctrl-d / ctrl-u move half of it (2 rows).
const SIZE: (u16, u16) = (80, 4);
const PR: &str = "https://github.com/acme/widgets/pull/7";

fn spec(name: &str) -> AgentSpec {
    AgentSpec {
        project: "p".to_owned(),
        name: name.to_owned(),
        source: std::path::PathBuf::new(),
        purpose: None,
        runner: Runner::Script,
        model: None,
        schedule: None,
        timezone: ScheduleTz::Utc,
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
        sandbox: CodexSandbox::Off,
        enabled: true,
        entrypoint: None,
        triggers: Vec::new(),
        allow_github_writes: false,
    }
}

fn run(id: &str) -> Run {
    Run {
        id: id.to_owned(),
        project: "p".to_owned(),
        agent: "a".to_owned(),
        state: RunState::Succeeded,
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

/// Three jobs (`p/a`, `p/b`, `p/c`); `p/a` has one run, `r1`.
fn app() -> App {
    let mut app = App::new();
    app.agents = vec![spec("a"), spec("b"), spec("c")];
    app.history = vec![run("r1")];
    app
}

/// `app()` on `p/a`'s runs screen, its step `r1` having opened [`PR`].
fn on_runs() -> App {
    let mut app = app();
    app.job_open = Some(("p".to_owned(), "a".to_owned()));
    app.step_pr = Some(("r1".to_owned(), Some(PR.to_owned())));
    app
}

fn log_view(lines: i64) -> LogView {
    LogView {
        run: run("r1"),
        events: (0..lines)
            .map(|i| agent_core::types::Event {
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
    }
}

fn press(app: &mut App, key: KeyEvent) -> Effect {
    input::apply(app, keys::map_key(key), SIZE)
}

fn char(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
}

fn copied(effect: Effect) -> String {
    match effect {
        Effect::Copy { text, .. } => text,
        other => panic!("expected a copy, got {other:?}"),
    }
}

/// Walks `beagle_tui::keymap::STANDARD` (`docs/CODING_STANDARDS.md`):
/// a new job fails to compile here until agenttop handles it or says why
/// it does not apply.
#[test]
fn agenttop_follows_the_standard_keymap() {
    use beagle_tui::keymap::{self, Job};
    for row in keymap::STANDARD {
        let mut app = app();
        let keys = keymap::keys(row.job);
        match row.job {
            Job::Move => {
                for (k, want) in keys.iter().zip([1, 0, 0, 2, 2, 0]) {
                    if *k == keys[4] {
                        app.dag_selected = 0;
                    }
                    press(&mut app, *k);
                    assert_eq!(app.dag_selected, want, "{k:?}");
                }
            }
            Job::OpenBack => {
                press(&mut app, keys[0]);
                assert!(app.runs_open(), "Enter opens the job's runs");
                press(&mut app, keys[1]);
                assert!(!app.runs_open(), "Esc goes back");
                press(&mut app, keys[0]);
                press(&mut app, keys[2]);
                assert!(!app.runs_open(), "b goes back");
            }
            // n/a: agenttop has no tabs (jobs → runs → log are levels,
            // entered with Enter), so Tab, ] and [ stay unbound.
            Job::Tabs => {
                for k in &keys {
                    assert_eq!(keys::map_key(*k), Action::None, "{k:?}");
                }
            }
            Job::HelpQuit => {
                press(&mut app, keys[0]);
                assert!(app.show_help, "?");
                assert_eq!(press(&mut app, char('j')), Effect::None);
                assert!(!app.show_help, "any key closes help");
                assert_eq!(app.dag_selected, 0, "and does nothing else");
                for k in &keys[1..] {
                    assert_eq!(press(&mut app, *k), Effect::Quit, "{k:?}");
                }
            }
            Job::Reload => assert_eq!(press(&mut app, keys[0]), Effect::Reload),
            Job::Search => {
                press(&mut app, keys[0]);
                assert!(app.job_search_input, "/ searches the jobs list");
            }
            Job::OpenInBrowser => {
                let mut app = on_runs();
                assert_eq!(press(&mut app, keys[0]), Effect::OpenUrl(PR.to_owned()));
            }
            Job::Edit => assert_eq!(press(&mut app, keys[0]), Effect::EditManifest),
            Job::Copy => {
                assert_eq!(copied(press(&mut app, keys[0])), "p/a", "y: the job");
                assert_eq!(copied(press(&mut app, keys[2])), "p/a", "i: its name");
                let mut app = on_runs();
                assert_eq!(copied(press(&mut app, keys[0])), "r1", "y: the step");
                assert_eq!(copied(press(&mut app, keys[1])), PR, "Y: its PR");
                assert_eq!(copied(press(&mut app, keys[2])), "r1", "i: its run id");
            }
            // n/a: agenttop has no side pane to resize; `w` reads the
            // selected step's write-up instead.
            Job::Resize => assert_eq!(keys::map_key(keys[0]), Action::WriteUp),
        }
    }
}

#[test]
fn keys_that_moved_for_the_standard() {
    let mut app = app();
    // f (was r): running jobs only; a: all again.
    press(&mut app, char('f'));
    assert!(app.job_running_only);
    press(&mut app, char('a'));
    assert!(!app.job_running_only);
    // L (was A, a capital reserved for a GitHub write): every job's runs.
    press(&mut app, char('L'));
    assert!(app.all_runs);
    // The old keys are free.
    for c in ['e', 'A'] {
        assert_eq!(keys::map_key(char(c)), Action::None, "{c}");
    }
    assert!(keys::HELP.iter().any(|(k, _)| *k == "L"));
    assert!(keys::HELP.iter().any(|(k, _)| *k == "E"));
}

#[test]
fn q_quits_from_any_level_and_esc_peels_one() {
    let mut app = on_runs();
    app.log_view = Some(log_view(3));
    app.show_run_detail = true;
    press(&mut app, char('b'));
    assert!(!app.show_run_detail, "the details overlay closes first");
    press(&mut app, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(app.log_view.is_none(), "then the log");
    assert!(app.runs_open(), "the runs screen is still there");
    app.log_view = Some(log_view(3));
    assert_eq!(press(&mut app, char('q')), Effect::Quit);
    let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    app.show_help = true;
    assert_eq!(
        press(&mut app, ctrl_c),
        Effect::Quit,
        "ctrl-c even over help"
    );
}

#[test]
fn g_and_capital_g_jump_through_a_log() {
    let mut app = app();
    // 40 one-line events in a 4-row frame: the body shows 1 line (4 rows
    // of chrome, never fewer than 1), so it scrolls 39.
    app.log_view = Some(log_view(40));
    let at = |app: &App| app.log_view.as_ref().map(|v| v.scroll_from_bottom);
    press(&mut app, char('g'));
    let top = at(&app).unwrap();
    assert!(top > 0, "g goes to the oldest line");
    press(&mut app, char('G'));
    assert_eq!(at(&app), Some(0), "G back to the newest");
    press(
        &mut app,
        KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL),
    );
    assert_eq!(
        at(&app),
        Some(1),
        "ctrl-u: half the body, at least one line"
    );
    press(
        &mut app,
        KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL),
    );
    assert_eq!(at(&app), Some(0));
    assert_eq!(copied(press(&mut app, char('i'))), "r1", "i: the run's id");
}

#[test]
fn without_a_selection_keys_explain_instead_of_acting() {
    let mut app = App::new();
    for c in ['y', 'i', 'Y', 'o'] {
        assert_eq!(press(&mut app, char(c)), Effect::None, "{c}");
        assert!(app.status_message().is_some(), "{c} says why");
    }
    let mut app = on_runs();
    app.step_pr = None;
    for c in ['o', 'Y', 'w'] {
        assert_eq!(press(&mut app, char(c)), Effect::None, "{c}");
        assert!(app.status_message().is_some(), "{c} says why");
    }
}

#[test]
fn other_ctrl_keys_do_nothing() {
    let ctrl_x = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL);
    assert_eq!(keys::map_key(ctrl_x), Action::None);
    assert_eq!(keys::map_key(char('z')), Action::None);
}
