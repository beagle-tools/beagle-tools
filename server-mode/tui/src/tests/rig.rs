//! A temp dir with fake programs and a fake [`Host`]: the tests never touch
//! this Mac's battery, screen lock, `beagle serve` or agentd.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use std::cell::{Cell, RefCell};
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::thread;
use std::time::{Duration, Instant};

use crate::cli::parse;
use crate::host::{Host, Svc};
use crate::procs::Programs;
use crate::session::Ctx;
use crate::state::StateDir;

/// The machine, as the tests want it.
#[derive(Debug)]
pub(super) struct FakeHost {
    pub battery: Cell<bool>,
    pub lock_ok: Cell<bool>,
    pub serve: Cell<Svc>,
    pub healthy: Cell<bool>,
    pub cert_days: Cell<i64>,
    pub agentd: Cell<Svc>,
    pub exposure: RefCell<Vec<String>>,
    /// Every call that changes something, in order.
    pub calls: RefCell<Vec<String>>,
}

impl Default for FakeHost {
    fn default() -> Self {
        Self {
            battery: Cell::new(false),
            lock_ok: Cell::new(true),
            serve: Cell::new(Svc::Stopped),
            healthy: Cell::new(true),
            cert_days: Cell::new(80),
            agentd: Cell::new(Svc::Running),
            exposure: RefCell::new(Vec::new()),
            calls: RefCell::new(Vec::new()),
        }
    }
}

impl FakeHost {
    fn call(&self, what: &str) {
        self.calls.borrow_mut().push(what.to_owned());
    }

    pub fn called(&self) -> Vec<String> {
        self.calls.borrow().clone()
    }
}

impl Host for FakeHost {
    fn on_battery(&self) -> bool {
        self.battery.get()
    }
    fn lock_screen(&self) -> bool {
        self.call("lock");
        self.lock_ok.get()
    }
    fn lock_delay(&self) -> Option<String> {
        Some("5 seconds".into())
    }
    fn display_sleep(&self) {
        self.call("display sleep");
    }
    fn serve(&self) -> Result<Svc, String> {
        Ok(self.serve.get())
    }
    fn serve_start(&self) -> Result<String, String> {
        self.call("serve start");
        self.serve.set(Svc::Running);
        Ok("started".into())
    }
    fn serve_stop(&self) -> Result<bool, String> {
        self.call("serve stop");
        let was = self.serve.get() == Svc::Running;
        self.serve.set(Svc::Stopped);
        Ok(was)
    }
    fn serve_health(&self) -> Result<(), String> {
        if self.serve.get() == Svc::Running && self.healthy.get() {
            Ok(())
        } else {
            Err("connection refused".into())
        }
    }
    fn serve_url(&self) -> Option<String> {
        Some("https://mac.example.net:8443".into())
    }
    fn new_pairing(&self) -> Result<(), String> {
        self.call("new pairing");
        Ok(())
    }
    fn cert_days_left(&self) -> Result<i64, String> {
        Ok(self.cert_days.get())
    }
    fn agentd(&self) -> Result<Svc, String> {
        Ok(self.agentd.get())
    }
    fn agentd_start(&self) -> Result<(), String> {
        self.call("agentd start");
        self.agentd.set(Svc::Running);
        Ok(())
    }
    fn exposure(&self) -> Vec<String> {
        self.exposure.borrow().clone()
    }
}

pub(super) struct Rig {
    pub tmp: tempfile::TempDir,
    pub ctx: Ctx,
    pub host: Rc<FakeHost>,
}

pub(super) fn script(path: &Path, body: &str) -> PathBuf {
    fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    path.to_path_buf()
}

impl Rig {
    pub fn new() -> Self {
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
        // Never run by the fake host; fail loudly if anything does.
        for name in ["osascript", "pmset", "sysadminctl"] {
            let fake = script(&bin.join(name), "exit 99");
            match name {
                "osascript" => progs.osascript = fake,
                "pmset" => progs.pmset = fake,
                _ => progs.sysadminctl = fake,
            }
        }
        let exe = script(&bin.join("beagle"), &log("beagle"));
        let host = Rc::new(FakeHost::default());
        let ctx = Ctx {
            state: StateDir::new(tmp.path().join("state")),
            legacy: Some(StateDir::new(tmp.path().join("legacy"))),
            progs,
            exe,
            config: Some(tmp.path().join("server-mode.toml")),
            host: host.clone(),
            health_wait: Duration::from_millis(600),
        };
        Self { tmp, ctx, host }
    }

    /// Writes `server-mode.toml`.
    pub fn config(&self, text: &str) {
        fs::write(self.ctx.config.as_ref().unwrap(), text).unwrap();
    }

    pub fn run(&self, args: &[&str]) -> Result<String, String> {
        let words: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
        let cmd = parse(&words)?;
        let mut out = Vec::new();
        self.ctx.exec(cmd, &mut out)?;
        Ok(String::from_utf8(out).unwrap())
    }

    pub fn calls(&self) -> String {
        fs::read_to_string(self.tmp.path().join("calls")).unwrap_or_default()
    }
}

pub(super) fn pid(dir: &StateDir, name: &str) -> u32 {
    dir.read_pid(name).unwrap_or_else(|| panic!("no {name}"))
}

/// Polls `check` for up to `secs` seconds.
pub(super) fn eventually(secs: u64, mut check: impl FnMut() -> bool) -> bool {
    let end = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < end {
        if check() {
            return true;
        }
        thread::sleep(Duration::from_millis(50));
    }
    check()
}
