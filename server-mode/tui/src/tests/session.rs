//! The commands end to end over a temp state dir, with a fake `caffeinate`
//! and a fake host (`rig.rs`): nothing real is started, stopped or locked.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use std::fs;
use std::process::Command;

use super::rig::{eventually, pid, script, Rig};
use crate::cli::{parse, Cmd};
use crate::procs::{alive, spawn_detached, stop, Programs, Target};

#[test]
fn stop_kills_a_whole_process_group() {
    let progs = Programs::system();
    let tmp = tempfile::tempdir().unwrap();
    let child_pid = tmp.path().join("child");
    let mut cmd = Command::new("sh");
    cmd.args([
        "-c",
        &format!("sleep 30 & echo $! > '{}'; wait", child_pid.display()),
    ]);
    let leader = spawn_detached(&mut cmd).unwrap();
    assert!(eventually(5, || child_pid.exists()));
    let child: u32 = fs::read_to_string(&child_pid)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert!(alive(&progs, Target::Group(leader)));
    stop(&progs, Target::Group(leader));
    assert!(eventually(5, || !alive(&progs, Target::Pid(leader))));
    assert!(eventually(5, || !alive(&progs, Target::Pid(child))));
}

#[test]
fn stop_kills_what_ignores_term() {
    let progs = Programs::system();
    let mut cmd = Command::new("sh");
    cmd.args(["-c", "trap '' TERM; while :; do sleep 0.1; done"]);
    let pid = spawn_detached(&mut cmd).unwrap();
    assert!(eventually(5, || alive(&progs, Target::Pid(pid))));
    stop(&progs, Target::Group(pid));
    assert!(eventually(5, || !alive(&progs, Target::Pid(pid))));
}

#[test]
fn on_status_log_off_round_trip() {
    let rig = Rig::new();
    let file = script(
        &rig.tmp.path().join("run.sh"),
        "echo started; exec sleep 30",
    );
    let out = rig
        .run(&["on", "-t", "2", "--start-up", "-f", file.to_str().unwrap()])
        .unwrap();
    assert_eq!(
        out,
        format!(
            "server mode on — awake for 2h\n  running {} (log: beagle server-mode log)\n  \
             ✓ beagle serve: answering at https://mac.example.net:8443\n  \
             ✓ certificate: 80 days left\n  ✓ agentd: running\n  \
             ✓ exposure: nothing found answering beyond this Mac\n  screen locked\n\
             Ready — open Beagle on your phone\n",
            file.display()
        )
    );
    assert!(eventually(5, || rig
        .calls()
        .contains("caffeinate -i -s -t 7200")));
    let state = &rig.ctx.state;
    let caf = pid(state, "caffeinate.pid");
    let timer = pid(state, "timer.pid");
    let startup = pid(state, "startup.pid");
    let progs = &rig.ctx.progs;
    assert!(alive(progs, Target::Pid(caf)));
    assert!(alive(progs, Target::Pid(timer)));
    assert_eq!(state.read("file"), Some(file.display().to_string()));

    let status = rig.run(&["status"]).unwrap();
    // 2h less the time since `on`: 2h00m within the same second, else 1h59m.
    assert!(
        ["1h59m", "2h00m"]
            .iter()
            .any(|left| status
                .starts_with(&format!("server mode: on (for 0h00m)\n  ends in {left} ("))),
        "{status}"
    );
    assert!(status.contains(&format!(
        "startup file: {} (running, pid {startup})",
        file.display()
    )));
    assert!(eventually(5, || rig.run(&["log"]).unwrap() == "started\n"));

    let again = rig.run(&["on"]).unwrap_err();
    assert!(again.starts_with("already on"), "{again}");

    assert_eq!(
        rig.run(&["off"]).unwrap(),
        "server mode off — the Mac can sleep again\n  beagle serve stopped\n"
    );
    assert_eq!(rig.host.called(), ["serve start", "lock", "serve stop"]);
    for pid in [caf, timer, startup] {
        assert!(eventually(5, || !alive(progs, Target::Pid(pid))), "{pid}");
    }
    assert_eq!(state.read("caffeinate.pid"), None);
    assert_eq!(rig.run(&["status"]).unwrap(), "server mode: off\n");
    assert_eq!(rig.run(&["off"]).unwrap(), "server mode was already off\n");
    assert_eq!(
        rig.run(&["log"]).unwrap(),
        "started\n",
        "the log survives off"
    );
}

#[test]
fn without_hours_there_is_no_timer() {
    let rig = Rig::new();
    let out = rig.run(&["--no-lock"]).unwrap();
    assert!(out.starts_with("server mode on — awake until 'beagle server-mode off'\n"));
    assert!(!out.contains("screen locked"));
    assert!(eventually(5, || rig.calls().contains("caffeinate -i -s\n")));
    assert_eq!(rig.ctx.state.read("timer.pid"), None);
    let status = rig.run(&["status"]).unwrap();
    assert!(status.contains("no end time — runs until 'beagle server-mode off'"));
    rig.run(&["off"]).unwrap();
}

#[test]
fn the_timer_runs_off_when_it_ends() {
    let rig = Rig::new();
    rig.run(&["on", "-t", "0.0001", "--no-lock"]).unwrap();
    assert!(eventually(10, || rig
        .calls()
        .contains("beagle server-mode off")));
    rig.run(&["off"]).unwrap();
}

#[test]
fn a_file_that_is_not_executable_runs_with_bash() {
    let rig = Rig::new();
    let file = rig.tmp.path().join("plain.sh");
    fs::write(&file, "echo \"via ${BASH_VERSION:+bash}\"\n").unwrap();
    rig.run(&["on", "--no-lock", "-f", file.to_str().unwrap()])
        .unwrap();
    assert!(eventually(5, || rig
        .run(&["log"])
        .is_ok_and(|l| l == "via bash\n")));
    let status = rig.run(&["status"]).unwrap();
    assert!(
        eventually(5, || rig.run(&["status"]).is_ok_and(
            |s| s.contains("(exited — see 'beagle server-mode log')")
        )),
        "{status}"
    );
    rig.run(&["off"]).unwrap();
}

#[test]
fn a_missing_file_is_refused_before_anything_starts() {
    let rig = Rig::new();
    let err = rig.run(&["on", "-f", "/nonexistent/run.sh"]).unwrap_err();
    assert_eq!(err, "no such file: /nonexistent/run.sh");
    assert_eq!(rig.ctx.state.read("caffeinate.pid"), None);
    assert!(!rig.calls().contains("caffeinate"));
}

#[test]
fn lock_falls_back_to_display_sleep() {
    let rig = Rig::new();
    rig.host.lock_ok.set(false);
    let out = rig.run(&["lock"]).unwrap();
    assert!(out.starts_with("  couldn't lock: give your terminal Accessibility access\n"));
    assert!(out.ends_with("it only locks after 5 seconds\n"), "{out}");
    assert_eq!(rig.host.called(), ["lock", "display sleep"]);
    assert_eq!(Rig::new().run(&["lock"]).unwrap(), "  screen locked\n");
}

#[test]
fn on_battery_is_said_whatever_this_mac_is_on() {
    let rig = Rig::new();
    rig.host.battery.set(true);
    let out = rig.run(&["--no-lock", "--no-serve"]).unwrap();
    assert!(out.contains("  on battery: it stays awake, but plug in for anything long\n"));
    rig.run(&["off"]).unwrap();
    let plugged = Rig::new();
    let out = plugged.run(&["--no-lock", "--no-serve"]).unwrap();
    assert!(!out.contains("on battery"), "{out}");
    plugged.run(&["off"]).unwrap();
}

#[test]
fn a_session_the_old_script_started_is_seen_and_stopped() {
    let rig = Rig::new();
    let legacy = rig.ctx.legacy.clone().unwrap();
    let caf = spawn_detached(Command::new("sleep").arg("30")).unwrap();
    legacy.write("caffeinate.pid", caf).unwrap();
    legacy.write("since", 0).unwrap();
    fs::write(legacy.path("startup.log"), "old log\n").unwrap();

    assert!(rig
        .run(&["status"])
        .unwrap()
        .starts_with("server mode: on ("));
    assert!(rig.run(&["on"]).unwrap_err().starts_with("already on"));
    assert_eq!(rig.run(&["log"]).unwrap(), "old log\n");
    assert_eq!(
        rig.run(&["off"]).unwrap(),
        "server mode off — the Mac can sleep again\n"
    );
    assert!(eventually(5, || !alive(&rig.ctx.progs, Target::Pid(caf))));
    assert_eq!(legacy.read("caffeinate.pid"), None);
}

#[test]
fn help_is_answered_without_touching_state() {
    let rig = Rig::new();
    assert_eq!(rig.run(&["help"]).unwrap(), "");
    assert!(matches!(parse(&["help".to_owned()]), Ok(Cmd::Help)));
    assert!(!rig.ctx.state.dir.exists());
}
