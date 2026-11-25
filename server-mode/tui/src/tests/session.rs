//! The commands end to end over a temp state dir, with fake `caffeinate`,
//! `osascript`, `pmset` and `sysadminctl`: the real ones never run here.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use crate::cli::{parse, Cmd};
use crate::procs::{alive, spawn_detached, stop, Programs, Target};
use crate::session::Ctx;
use crate::state::StateDir;

/// A temp dir with fake programs and two state dirs.
struct Rig {
    tmp: tempfile::TempDir,
    ctx: Ctx,
}

fn script(path: &Path, body: &str) -> PathBuf {
    fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    path.to_path_buf()
}

impl Rig {
    /// `osascript` fails (no Accessibility) unless `lock_ok`.
    fn new(lock_ok: bool) -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let bin = tmp.path().join("bin");
        fs::create_dir(&bin).unwrap();
        let rec = tmp.path().join("calls");
        let log = |name: &str| format!("echo \"{name} $*\" >> '{}'", rec.display());
        let mut progs = Programs::system();
        progs.caffeinate = script(
            &bin.join("caffeinate"),
            &format!("{}\nexec sleep 30", log("caffeinate")),
        );
        progs.osascript = script(
            &bin.join("osascript"),
            &format!("{}\nexit {}", log("osascript"), u8::from(!lock_ok)),
        );
        progs.pmset = script(
            &bin.join("pmset"),
            &format!(
                "{}\n[ \"$1\" = -g ] && echo \"Now drawing from 'Battery Power'\"\nexit 0",
                log("pmset")
            ),
        );
        progs.sysadminctl = script(
            &bin.join("sysadminctl"),
            "echo 'screenLock delay is 5 seconds' >&2",
        );
        let exe = script(&bin.join("beagle"), &log("beagle"));
        let ctx = Ctx {
            state: StateDir::new(tmp.path().join("state")),
            legacy: Some(StateDir::new(tmp.path().join("legacy"))),
            progs,
            exe,
        };
        Self { tmp, ctx }
    }

    fn run(&self, args: &[&str]) -> Result<String, String> {
        let words: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
        let cmd = parse(&words)?;
        let mut out = Vec::new();
        self.ctx.exec(cmd, &mut out)?;
        Ok(String::from_utf8(out).unwrap())
    }

    fn calls(&self) -> String {
        fs::read_to_string(self.tmp.path().join("calls")).unwrap_or_default()
    }
}

fn pid(dir: &StateDir, name: &str) -> u32 {
    dir.read_pid(name).unwrap_or_else(|| panic!("no {name}"))
}

/// Polls `check` for up to `secs` seconds.
fn eventually(secs: u64, mut check: impl FnMut() -> bool) -> bool {
    let end = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < end {
        if check() {
            return true;
        }
        thread::sleep(Duration::from_millis(50));
    }
    check()
}

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
    let rig = Rig::new(true);
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
             on battery: it stays awake, but plug in for anything long\n  screen locked\n",
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
    assert!(
        status.starts_with("server mode: on (for 0h00m)\n  ends in 1h59m"),
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
        "server mode off — the Mac can sleep again\n"
    );
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
    let rig = Rig::new(true);
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
    let rig = Rig::new(true);
    rig.run(&["on", "-t", "0.0001", "--no-lock"]).unwrap();
    assert!(eventually(10, || rig
        .calls()
        .contains("beagle server-mode off")));
    rig.run(&["off"]).unwrap();
}

#[test]
fn a_file_that_is_not_executable_runs_with_bash() {
    let rig = Rig::new(true);
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
    let rig = Rig::new(true);
    let err = rig.run(&["on", "-f", "/nonexistent/run.sh"]).unwrap_err();
    assert_eq!(err, "no such file: /nonexistent/run.sh");
    assert_eq!(rig.ctx.state.read("caffeinate.pid"), None);
    assert!(!rig.calls().contains("caffeinate"));
}

#[test]
fn lock_falls_back_to_display_sleep() {
    let rig = Rig::new(false);
    let out = rig.run(&["lock"]).unwrap();
    assert!(out.starts_with("  couldn't lock: give your terminal Accessibility access\n"));
    assert!(out.ends_with("it only locks after 5 seconds\n"), "{out}");
    assert!(rig.calls().contains("pmset displaysleepnow"));
    assert_eq!(Rig::new(true).run(&["lock"]).unwrap(), "  screen locked\n");
}

#[test]
fn a_session_the_old_script_started_is_seen_and_stopped() {
    let rig = Rig::new(true);
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
    let rig = Rig::new(true);
    assert_eq!(rig.run(&["help"]).unwrap(), "");
    assert!(matches!(parse(&["help".to_owned()]), Ok(Cmd::Help)));
    assert!(!rig.ctx.state.dir.exists());
}
