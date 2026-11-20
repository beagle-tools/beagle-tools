//! The gh account writes go out as: never gh's active one by default.

use beagle_utils::gh::Account;

use super::super::account::resolve;
use super::super::worker::{acting_as, Cmd, Done, Job};
use super::*;

fn acct(login: &str, active: bool) -> Account {
    Account {
        login: login.to_owned(),
        active,
    }
}

/// gh logged into two accounts, the work one active.
fn two() -> Vec<Account> {
    vec![acct("octo-work", true), acct("octo-personal", false)]
}

/// The fixture app with no account configured, a fake worker, and gh's
/// answer (when given) already drained. Returns the jobs the UI sends.
fn unconfigured(accounts: Option<Vec<Account>>) -> (App, std::sync::mpsc::Receiver<Job>) {
    let mut app = app();
    app.config.github_user = None;
    app.account = None;
    let (jobs, done) = app.fake_worker();
    if let Some(list) = accounts {
        done.send(Done::Accounts(list)).unwrap();
        assert!(app.drain_worker());
    }
    (app, jobs)
}

fn to_comments(app: &mut App) {
    press(app, KeyCode::Enter);
    press(app, KeyCode::Char('3'));
}

#[test]
fn resolve_never_picks_the_active_account() {
    assert_eq!(
        resolve(Some("octo-personal"), Some(&two())).as_deref(),
        Some("octo-personal")
    );
    assert_eq!(resolve(None, Some(&two())), None, "two accounts: ask");
    assert_eq!(
        resolve(None, Some(&[acct("octo-solo", true)])).as_deref(),
        Some("octo-solo")
    );
    assert_eq!(resolve(None, Some(&[])), None);
    assert_eq!(resolve(None, None), None, "gh has not answered");
    assert_eq!(resolve(Some(""), Some(&two())), None);
}

#[test]
fn two_accounts_and_no_config_ask_before_the_first_post() {
    let (mut app, jobs) = unconfigured(Some(two()));
    let dir = std::env::temp_dir().join(format!("tandem-account-{}", uuid::Uuid::new_v4()));
    let file = dir.join("beagle/tandem.toml");
    app.config_file = Some(file.clone());
    assert!(draw(&mut app).contains("as ? (u to choose)"));

    to_comments(&mut app);
    press(&mut app, KeyCode::Char('P'));
    let Mode::Pick {
        sel: 0,
        then: Some(Action::Post { .. }),
    } = &app.mode
    else {
        unreachable!("P with no account must ask who posts: {:?}", app.mode);
    };
    assert!(draw(&mut app).contains("Post to GitHub as which account?"));
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Enter);
    // Chosen: now the write's own confirmation, naming the account.
    assert!(matches!(app.mode, Mode::Confirm(Action::Post { .. })));
    assert_eq!(app.account.as_deref(), Some("octo-personal"));
    let screen = draw(&mut app);
    assert!(screen.contains("as octo-personal?"), "confirm title");

    // Saved for next time.
    let saved = std::fs::read_to_string(&file).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(saved, "github_user = \"octo-personal\"\n");

    press(&mut app, KeyCode::Char('y'));
    let job = jobs.try_recv().unwrap();
    let Cmd::Post { user, .. } = &job.cmd else {
        unreachable!("expected a post: {job:?}");
    };
    assert_eq!(user, "octo-personal");
    assert_eq!(acting_as(&job).as_deref(), Some("octo-personal"));
    // The footer names who posts from now on.
    app.status = None;
    assert!(draw(&mut app).contains(" as octo-personal "));
}

#[test]
fn cancelling_the_picker_writes_nothing() {
    let (mut app, jobs) = unconfigured(Some(two()));
    press(&mut app, KeyCode::Char('A'));
    assert!(matches!(
        app.mode,
        Mode::Pick {
            then: Some(Action::Approve(_)),
            ..
        }
    ));
    press(&mut app, KeyCode::Esc);
    assert_eq!(app.mode, Mode::Normal);
    assert_eq!(app.account, None);
    assert!(jobs.try_recv().is_err(), "nothing sent");
}

#[test]
fn one_account_is_used_without_asking() {
    let (mut app, jobs) = unconfigured(Some(vec![acct("octo-solo", true)]));
    assert_eq!(app.account.as_deref(), Some("octo-solo"));
    press(&mut app, KeyCode::Char('A'));
    assert!(matches!(app.mode, Mode::Confirm(Action::Approve(_))));
    press(&mut app, KeyCode::Char('y'));
    let job = jobs.try_recv().unwrap();
    assert!(
        matches!(&job.cmd, Cmd::Approve { user, .. } if user == "octo-solo"),
        "{job:?}"
    );
}

#[test]
fn no_answer_from_gh_yet_or_no_account_never_writes() {
    let (mut app, jobs) = unconfigured(None);
    press(&mut app, KeyCode::Char('A'));
    assert_eq!(app.mode, Mode::Normal);
    assert!(
        matches!(jobs.try_recv().unwrap().cmd, Cmd::Accounts),
        "asks gh again"
    );
    assert!(draw(&mut app).contains("as … "));

    let (mut app, jobs) = unconfigured(Some(Vec::new()));
    press(&mut app, KeyCode::Char('A'));
    assert_eq!(app.mode, Mode::Normal);
    let (msg, error) = app.status.clone().unwrap();
    assert!(error && msg.contains("gh auth login"), "{msg}");
    assert!(jobs.try_recv().is_err());
}

#[test]
fn u_switches_the_account_and_reads_follow_it() {
    let (mut app, jobs) = unconfigured(Some(two()));
    app.account = Some("octo-personal".into());
    press(&mut app, KeyCode::Char('u'));
    // Opens on the current account; nothing pending.
    assert_eq!(app.mode, Mode::Pick { sel: 1, then: None });
    press(&mut app, KeyCode::Char('k'));
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.mode, Mode::Normal);
    assert_eq!(app.account.as_deref(), Some("octo-work"));
    // No config file in tests: kept for the session, and it says so.
    let (msg, error) = app.status.clone().unwrap();
    assert!(error && msg.contains("for this session"), "{msg}");
    press(&mut app, KeyCode::Char('r'));
    let job = jobs.try_recv().unwrap();
    assert!(matches!(job.cmd, Cmd::Fetch(_)));
    assert_eq!(acting_as(&job).as_deref(), Some("octo-work"));
    // ctrl-u is still the half-page scroll, not the picker.
    app.handle_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
    assert_eq!(app.mode, Mode::Normal);
}

#[test]
fn a_configured_account_gh_does_not_have_is_flagged() {
    let mut app = app();
    let (_jobs, done) = app.fake_worker();
    done.send(Done::Accounts(two())).unwrap();
    app.account = Some("octo-gone".into());
    app.drain_worker();
    let (msg, error) = app.status.clone().unwrap();
    assert!(
        error && msg.contains("octo-gone is not logged in to gh"),
        "{msg}"
    );
}

#[test]
fn u_is_a_lowercase_key_in_the_help() {
    assert!(super::super::keys::HELP.iter().any(|(k, _)| *k == "u"));
}
