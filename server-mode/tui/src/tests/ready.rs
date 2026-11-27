//! `server-mode.toml`, the before/after steps, `beagle serve` and the
//! checklist, over the fake host.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use std::fs;
use std::path::PathBuf;

use super::rig::{eventually, script, Rig};
use crate::cli::OnOpts;
use crate::config::{parse, plan, Config};
use crate::host::Svc;
use crate::report::parse_enddate;

#[test]
fn the_config_file_is_all_optional_and_strict() {
    assert_eq!(parse("").unwrap(), Config::default());
    let config = parse(
        r#"
serve = false
hours = 1.5
on_stop = "~/stop.sh"
[[before]]
name = "network"
run = "vpn up"
wait = "vpn ip"
timeout = 5
"#,
    )
    .unwrap();
    assert_eq!(config.serve, Some(false));
    assert_eq!(config.before[0].wait.as_deref(), Some("vpn ip"));
    assert_eq!(config.before[0].timeout_secs(), 5);
    assert!(!config.before[0].optional);
    assert!(parse("sevre = true").is_err(), "typos fail loudly");
    assert!(
        parse("[[after]]\nname = \"x\"").is_err(),
        "a step needs run"
    );
}

#[test]
fn flags_win_over_the_file_and_the_file_over_defaults() {
    let defaults = plan(&OnOpts::default(), &Config::default()).unwrap();
    assert!(defaults.serve && defaults.lock && !defaults.new_pairing);
    assert_eq!(defaults.secs, None);
    let config = parse("serve = false\nlock = false\nhours = 2\nnew_pairing = true").unwrap();
    let from_file = plan(&OnOpts::default(), &config).unwrap();
    assert!(!from_file.serve && !from_file.lock && from_file.new_pairing);
    assert_eq!(from_file.secs, Some(7200));
    let flags = OnOpts {
        hours: Some("0.5".into()),
        secs: Some(1800),
        serve: Some(true),
        lock: Some(true),
        file: Some(PathBuf::from("run.sh")),
        ..OnOpts::default()
    };
    let both = plan(&flags, &config).unwrap();
    assert!(both.serve && both.lock);
    assert_eq!(both.secs, Some(1800));
    assert_eq!(both.file, Some(PathBuf::from("run.sh")));
}

#[test]
fn before_steps_run_in_order_and_wait_until_ready() {
    let rig = Rig::new();
    let flag = rig.tmp.path().join("up");
    rig.config(&format!(
        "[[before]]\nname = \"one\"\nrun = \"echo first\"\n\
         [[before]]\nname = \"network\"\nrun = \"(sleep 1; touch '{}') &\"\nwait = \"test -e '{}'\"\ntimeout = 10\n",
        flag.display(),
        flag.display()
    ));
    let out = rig.run(&["on", "--no-lock"]).unwrap();
    assert!(
        out.contains("  ✓ step one\n  ✓ step network\n  ✓ beagle serve"),
        "{out}"
    );
    assert!(
        out.ends_with("Ready — open Beagle on your phone\n"),
        "{out}"
    );
    let log = rig.run(&["log"]).unwrap();
    assert!(
        log.starts_with("== one: echo first\nfirst\n== network:"),
        "{log}"
    );
    rig.run(&["off"]).unwrap();
}

#[test]
fn a_failing_step_stops_on_before_anything_starts() {
    let rig = Rig::new();
    rig.config("[[before]]\nname = \"network\"\nrun = \"echo 'log in first' >&2; exit 3\"\n");
    let err = rig.run(&["on"]).unwrap_err();
    assert!(
        err.starts_with(
            "step 'network' failed: `echo 'log in first' >&2; exit 3` exited 3: log in first\n"
        ),
        "{err}"
    );
    assert_eq!(rig.ctx.state.read("caffeinate.pid"), None);
    assert_eq!(
        rig.host.called(),
        Vec::<String>::new(),
        "serve untouched, no lock"
    );
    assert!(!rig.calls().contains("caffeinate"));
}

#[test]
fn an_optional_step_only_warns_and_a_wait_times_out() {
    let rig = Rig::new();
    rig.config(
        "[[before]]\nname = \"maybe\"\nrun = \"true\"\nwait = \"false\"\ntimeout = 1\noptional = true\n",
    );
    let out = rig.run(&["on", "--no-lock"]).unwrap();
    assert!(
        out.contains("  ⚠ step maybe: `false` still failing after 1s (exited 1)\n"),
        "{out}"
    );
    assert!(out.ends_with("Ready — open Beagle on your phone\n"));
    rig.run(&["off"]).unwrap();
}

#[test]
fn off_stops_serve_then_runs_on_stop_and_after_steps() {
    let rig = Rig::new();
    let done = rig.tmp.path().join("done");
    let on_stop = script(
        &rig.tmp.path().join("stop.sh"),
        &format!("echo stopping >> '{}'", done.display()),
    );
    rig.config(&format!(
        "[[after]]\nname = \"tidy\"\nrun = \"echo tidy >> '{}'\"\n",
        done.display()
    ));
    rig.run(&["on", "--no-lock", "--on-stop", on_stop.to_str().unwrap()])
        .unwrap();
    assert_eq!(rig.host.serve.get(), Svc::Running);
    let out = rig.run(&["off"]).unwrap();
    assert_eq!(
        out,
        "server mode off — the Mac can sleep again\n  beagle serve stopped\n  \
         ✓ step on-stop\n  ✓ step tidy\n"
    );
    assert_eq!(rig.host.serve.get(), Svc::Stopped);
    assert_eq!(fs::read_to_string(&done).unwrap(), "stopping\ntidy\n");
}

#[test]
fn no_serve_leaves_beagle_serve_alone() {
    let rig = Rig::new();
    rig.host.serve.set(Svc::Running);
    let out = rig.run(&["on", "--no-lock", "--no-serve"]).unwrap();
    assert!(!out.contains("beagle serve:"), "{out}");
    assert_eq!(
        rig.run(&["off"]).unwrap(),
        "server mode off — the Mac can sleep again\n"
    );
    assert_eq!(rig.host.called(), Vec::<String>::new());
    assert_eq!(rig.host.serve.get(), Svc::Running);
}

#[test]
fn a_server_that_never_answers_is_not_ready() {
    let rig = Rig::new();
    rig.host.healthy.set(false);
    rig.host.cert_days.set(9);
    rig.host.agentd.set(Svc::NotInstalled);
    rig.host
        .exposure
        .borrow_mut()
        .push("Remote Login accepts passwords".into());
    let out = rig.run(&["on", "--no-lock", "--new-pairing"]).unwrap();
    assert!(out.contains(
        "  ✗ beagle serve: running but https://mac.example.net:8443/v1/health fails (connection refused)"
    ));
    assert!(out.contains("  ⚠ certificate: 9 days left — renew it, then `beagle serve install`\n"));
    assert!(out.contains("  ⚠ agentd: not installed"));
    assert!(out.contains("  ⚠ exposure: Remote Login accepts passwords\n"));
    assert!(out.ends_with("Not ready — fix the ✗ lines above, then 'beagle server-mode status'\n"));
    assert_eq!(rig.host.called(), ["new pairing", "serve start"]);
    rig.run(&["off"]).unwrap();
}

#[test]
fn status_starts_a_stopped_agentd_and_shows_the_checklist() {
    let rig = Rig::new();
    rig.run(&["on", "--no-lock"]).unwrap();
    rig.host.agentd.set(Svc::Stopped);
    let status = rig.run(&["status"]).unwrap();
    assert!(
        status.contains("  ✓ beagle serve: answering at"),
        "{status}"
    );
    assert!(status.contains("  ✓ agentd: started\n"), "{status}");
    assert!(rig.host.called().contains(&"agentd start".to_owned()));
    rig.run(&["off"]).unwrap();
}

#[test]
fn status_finishes_a_session_that_ended_unseen() {
    let rig = Rig::new();
    rig.run(&["on", "--no-lock"]).unwrap();
    // The Mac slept through the timer: caffeinate is gone, serve still up.
    let caf = rig.ctx.state.read_pid("caffeinate.pid").unwrap();
    crate::procs::stop(&rig.ctx.progs, crate::procs::Target::Pid(caf));
    assert!(eventually(5, || !crate::procs::alive(
        &rig.ctx.progs,
        crate::procs::Target::Pid(caf)
    )));
    let out = rig.run(&["status"]).unwrap();
    assert!(
        out.starts_with("server mode had ended — finishing the stop\n"),
        "{out}"
    );
    assert!(out.contains("  beagle serve stopped\n"));
    assert_eq!(rig.host.serve.get(), Svc::Stopped);
    assert_eq!(rig.run(&["status"]).unwrap(), "server mode: off\n");
}

#[test]
fn certificate_end_dates_parse() {
    assert_eq!(
        parse_enddate("notAfter=Jan  1 00:00:00 2030 GMT\n"),
        Some(1_893_456_000)
    );
    assert_eq!(parse_enddate("notAfter=Foo 1 00:00:00 2030 GMT"), None);
    assert_eq!(parse_enddate("garbage"), None);
}
