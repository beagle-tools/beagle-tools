//! Tests for `beagle setup`: argument parsing, step picking, and the
//! runner against a temp HOME (never the real one). The tool commands are
//! recorded by a fake runner, not executed, except where a test installs
//! for real into the temp HOME.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use std::ffi::OsString;

use super::*;

fn args(list: &[&str]) -> Vec<OsString> {
    list.iter().map(OsString::from).collect()
}

#[test]
fn parses_yes_skip_and_only() {
    let opts = Options::parse(&args(&["--yes", "--skip", "scout-hooks"])).unwrap();
    assert!(opts.yes);
    assert_eq!(opts.skip, vec!["scout-hooks"]);
    let opts = Options::parse(&args(&["--only", "rca-skills,scout-skills"])).unwrap();
    assert_eq!(
        opts.only,
        Some(vec!["rca-skills".to_owned(), "scout-skills".to_owned()])
    );
}

#[test]
fn rejects_unknown_steps_and_flags() {
    assert!(Options::parse(&args(&["--skip", "nope"]))
        .unwrap_err()
        .contains("unknown step `nope`"));
    assert!(Options::parse(&args(&["--skip"])).is_err());
    assert!(Options::parse(&args(&["--force"]))
        .unwrap_err()
        .contains("unknown argument"));
}

#[test]
fn step_ids_are_unique() {
    let mut ids: Vec<&str> = STEPS.iter().map(|s| s.id).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), STEPS.len());
}

#[test]
fn yes_runs_every_step_a_fresh_home_needs() {
    let home = tempfile::tempdir().expect("tempdir");
    let opts = Options {
        yes: true,
        ..Options::default()
    };
    let mut ran = Vec::new();
    let outcomes = run_steps(
        &opts,
        home.path(),
        |tool, args| {
            ran.push(format!("{tool} {}", args.join(" ")));
            true
        },
        |_| panic!("--yes never asks"),
    );
    assert_eq!(ran[0], "rca skill install");
    assert_eq!(ran[1], "scout skill install");
    assert_eq!(ran[2], "scout hook install");
    // The store step depends on $SCOUT_ROOT / scout's config on this
    // machine; when it runs, it inits ~/.scout under the temp HOME.
    if let Some(store) = ran.get(3) {
        assert_eq!(
            *store,
            format!("scout init {}", home.path().join(".scout").display())
        );
    }
    assert!(outcomes
        .iter()
        .all(|(_, o)| matches!(o, Outcome::Ran | Outcome::AlreadyDone)));
}

#[test]
fn skip_only_and_a_declined_prompt_leave_steps_out() {
    let home = tempfile::tempdir().expect("tempdir");
    let opts = Options {
        only: Some(vec!["rca-skills".to_owned(), "scout-skills".to_owned()]),
        skip: vec!["scout-skills".to_owned()],
        ..Options::default()
    };
    let mut ran = Vec::new();
    let mut asked = 0;
    let outcomes = run_steps(
        &opts,
        home.path(),
        |tool, _| {
            ran.push(tool.to_owned());
            true
        },
        |_| {
            asked += 1;
            false // decline
        },
    );
    assert!(ran.is_empty(), "declined: {ran:?}");
    assert_eq!(asked, 1, "only rca-skills is asked about");
    assert!(outcomes.iter().all(|(_, o)| *o == Outcome::Skipped));
}

#[test]
fn a_failing_step_is_reported_and_the_rest_still_run() {
    let home = tempfile::tempdir().expect("tempdir");
    let opts = Options {
        yes: true,
        only: Some(vec!["rca-skills".to_owned(), "scout-hooks".to_owned()]),
        ..Options::default()
    };
    let outcomes = run_steps(&opts, home.path(), |tool, _| tool != "rca", |_| true);
    assert_eq!(outcomes[0], ("rca-skills", Outcome::Failed));
    assert_eq!(outcomes[2], ("scout-hooks", Outcome::Ran));
}

#[test]
fn installed_skills_count_as_done() {
    let home = tempfile::tempdir().expect("tempdir");
    assert!(!rca_skills_current(home.path()));
    assert!(!scout_skills_current(home.path()));
    for agent in rca_skill::Agent::ALL {
        for skill in rca_skill::Skill::ALL {
            rca_skill::install(skill, agent, home.path()).unwrap();
        }
    }
    for agent in scout_skill::Agent::ALL {
        for skill in scout_skill::Skill::ALL {
            agent.install(skill, home.path()).unwrap();
        }
    }
    assert!(rca_skills_current(home.path()));
    assert!(scout_skills_current(home.path()));
    assert!(!scout_hooks_installed(home.path()));
}
