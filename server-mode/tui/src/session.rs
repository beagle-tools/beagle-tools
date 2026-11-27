//! The commands: `on` and `off` (`status`, `log` and `lock` are in
//! `report.rs`).

use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::rc::Rc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::cli::{Cmd, OnOpts};
use crate::config::{self, Step};
use crate::host::{Host, Svc};
use crate::procs::{alive, spawn_detached, stop, Programs, Target};
use crate::report::{Line, Mark};
use crate::state::{StateDir, LOG_FILE};
use crate::steps;

/// What the timer runs: sleep, then become `<beagle> server-mode off`
/// (`exec`, so `off` sees its own pid in `timer.pid` and spares itself).
const TIMER_SCRIPT: &str = r#"sleep "$1"; exec "$2" server-mode off"#;

/// How long `on` waits for `beagle serve` to answer its health check.
pub(crate) const HEALTH_WAIT: Duration = Duration::from_secs(30);

/// Everything a command needs; tests build one over temp dirs and fakes.
#[derive(Debug, Clone)]
pub(crate) struct Ctx {
    /// Where this tool keeps its session.
    pub state: StateDir,
    /// The old script's state dir, read so its session isn't lost.
    pub legacy: Option<StateDir>,
    pub progs: Programs,
    /// The `beagle` binary the timer runs as `<exe> server-mode off`.
    pub exe: PathBuf,
    /// `server-mode.toml` (a missing file is an empty config).
    pub config: Option<PathBuf>,
    pub host: Rc<dyn Host>,
    /// How long `on` waits for the server's health check.
    pub health_wait: Duration,
}

pub(crate) type Res = Result<(), String>;

pub(crate) fn io_err(what: &str) -> impl Fn(io::Error) -> String + '_ {
    move |e| format!("{what}: {e}")
}

pub(crate) fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

impl Ctx {
    /// The real thing: beagle's state dir, the old one, programs on PATH.
    pub fn system() -> Result<Self, String> {
        let state = crate::state::default_dir().ok_or("no home directory")?;
        let legacy = beagle_utils::paths::home_dir().map(|h| crate::state::legacy_dir(&h));
        let exe = std::env::current_exe().map_err(io_err("can't find the beagle binary"))?;
        let exe = beagle_utils::paths::stable_exe(&exe);
        let progs = Programs::system();
        Ok(Self {
            state: StateDir::new(state),
            legacy: legacy.map(StateDir::new),
            host: Rc::new(crate::host::System {
                progs: progs.clone(),
                exe: exe.clone(),
            }),
            progs,
            exe,
            config: config::default_path(),
            health_wait: HEALTH_WAIT,
        })
    }

    pub fn exec(&self, cmd: Cmd, out: &mut dyn Write) -> Res {
        let res = match cmd {
            Cmd::On(opts) => self.on(&opts, out),
            Cmd::Off => self.off(out),
            Cmd::Status => self.status(out),
            Cmd::Log => self.log(out),
            Cmd::Lock => self.lock_screen(out),
            Cmd::Help => Ok(()),
        };
        res.and_then(|()| out.flush().map_err(io_err("write")))
    }

    /// This tool's dir, then the old script's.
    pub(crate) fn dirs(&self) -> impl Iterator<Item = &StateDir> {
        std::iter::once(&self.state).chain(self.legacy.as_ref())
    }

    fn is_on(&self, dir: &StateDir) -> bool {
        dir.read_pid("caffeinate.pid")
            .is_some_and(|p| alive(&self.progs, Target::Pid(p)))
    }

    /// The dir holding the running session, if any.
    pub(crate) fn active(&self) -> Option<&StateDir> {
        self.dirs().find(|d| self.is_on(d))
    }

    fn load_config(&self) -> Result<config::Config, String> {
        self.config
            .as_deref()
            .map_or_else(|| Ok(config::Config::default()), config::load)
    }

    fn on(&self, opts: &OnOpts, out: &mut dyn Write) -> Res {
        if self.active().is_some() {
            return Err("already on — run 'beagle server-mode off' first \
                        (or 'beagle server-mode status')"
                .into());
        }
        let plan = config::plan(opts, &self.load_config()?)?;
        let file = plan.file.as_deref().map(absolute_file).transpose()?;
        let on_stop = plan.on_stop.as_deref().map(absolute_file).transpose()?;
        // Clear stale state from a crash or reboot (its serve is restarted
        // below anyway, so nothing else of it needs finishing).
        for dir in self.dirs() {
            self.stop_session(dir);
        }
        let state = &self.state;
        fs::create_dir_all(&state.dir).map_err(io_err("create the state dir"))?;
        fs::File::create(state.path(LOG_FILE)).map_err(io_err("create the log"))?;
        let w = io_err("write");
        let mut lines = Vec::new();

        for step in &plan.before {
            match steps::run(&self.progs.sh, step, &state.path(LOG_FILE)) {
                Ok(()) => lines.push(Line::ok(format!("step {}", step.name))),
                Err(e) if step.optional => {
                    lines.push(Line::warn(format!("step {}: {e}", step.name)));
                }
                Err(e) => {
                    return Err(format!(
                    "step '{}' failed: {e}\n  nothing was started (see 'beagle server-mode log')",
                    step.name
                ))
                }
            }
        }

        if plan.serve {
            state.write("serve", 1).map_err(io_err("write state"))?;
            if plan.new_pairing {
                self.host.new_pairing()?;
            }
            if let Err(e) = self.host.serve_start() {
                lines.push(Line::fail(format!("beagle serve: couldn't start: {e}")));
            } else {
                self.wait_healthy();
            }
        }
        if let Some(path) = &on_stop {
            state
                .write("on_stop", path.display())
                .map_err(io_err("write state"))?;
        }

        let start = now();
        state.write("since", start).map_err(io_err("write state"))?;
        self.start_awake(start, plan.secs)?;
        if let Some(file) = &file {
            self.start_file(file)?;
        }

        let until = match &plan.hours {
            Some(h) => format!(" for {h}h"),
            None => " until 'beagle server-mode off'".into(),
        };
        writeln!(out, "server mode on — awake{until}").map_err(&w)?;
        if let Some(file) = &file {
            writeln!(
                out,
                "  running {} (log: beagle server-mode log)",
                file.display()
            )
            .map_err(&w)?;
        }
        if self.host.on_battery() {
            writeln!(
                out,
                "  on battery: it stays awake, but plug in for anything long"
            )
            .map_err(&w)?;
        }
        lines.extend(self.checks(plan.serve));
        for line in &lines {
            writeln!(out, "  {line}").map_err(&w)?;
        }
        if plan.lock {
            self.lock_screen(out)?;
        }
        let ready = if lines.iter().any(|l| l.mark == Mark::Fail) {
            "Not ready — fix the ✗ lines above, then 'beagle server-mode status'"
        } else {
            "Ready — open Beagle on your phone"
        };
        writeln!(out, "{ready}").map_err(&w)
    }

    /// Polls the server's health check for up to [`Ctx::health_wait`].
    fn wait_healthy(&self) {
        let end = Instant::now() + self.health_wait;
        while self.host.serve_health().is_err() && Instant::now() < end {
            thread::sleep(Duration::from_millis(500));
        }
    }

    /// `caffeinate -i -s`, and the timer when `secs` is set.
    fn start_awake(&self, start: i64, secs: Option<u64>) -> Res {
        let state = &self.state;
        // -t is a backstop: the timer also stops the startup file, but if
        // it dies caffeinate still lets the Mac sleep on schedule.
        let mut caf = self.progs.nohup(&self.progs.caffeinate);
        caf.args(["-i", "-s"]);
        if let Some(secs) = secs {
            caf.args(["-t", &secs.to_string()]);
        }
        caf.stdout(Stdio::null()).stderr(Stdio::null());
        let pid = spawn_detached(&mut caf).map_err(io_err("couldn't start caffeinate"))?;
        state
            .write("caffeinate.pid", pid)
            .map_err(io_err("write state"))?;
        let Some(secs) = secs else {
            return Ok(());
        };
        let until = start.saturating_add(i64::try_from(secs).unwrap_or(i64::MAX));
        state.write("until", until).map_err(io_err("write state"))?;
        let mut timer = self.progs.nohup(&self.progs.sh);
        timer
            .args(["-c", TIMER_SCRIPT, "_", &secs.to_string()])
            .arg(&self.exe)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let pid = spawn_detached(&mut timer).map_err(io_err("couldn't start the timer"))?;
        state.write("timer.pid", pid).map_err(io_err("write state"))
    }

    /// Runs the startup file in its own process group, output to the log.
    /// Executable files run as-is (their shebang wins), others with bash.
    fn start_file(&self, file: &Path) -> Res {
        let state = &self.state;
        state
            .write("file", file.display())
            .map_err(io_err("write state"))?;
        let open = || {
            fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(state.path(LOG_FILE))
                .map_err(io_err("open the log"))
        };
        let executable = fs::metadata(file).is_ok_and(|m| m.permissions().mode() & 0o111 != 0);
        let mut cmd = if executable {
            self.progs.nohup(file)
        } else {
            let mut c = self.progs.nohup(&self.progs.bash);
            c.arg(file);
            c
        };
        cmd.stdout(open()?).stderr(open()?);
        let pid = spawn_detached(&mut cmd)
            .map_err(|e| format!("couldn't start {}: {e}", file.display()))?;
        state
            .write("startup.pid", pid)
            .map_err(io_err("write state"))
    }

    /// `off`: stops the session, then `beagle serve` (if `on` started it),
    /// then runs the `--on-stop` file and the `[[after]]` steps.
    pub(crate) fn off(&self, out: &mut dyn Write) -> Res {
        let w = io_err("write");
        let was_on = self.active().is_some();
        let managed = self.state.read("serve").is_some();
        let on_stop = self.state.read("on_stop");
        for dir in self.dirs() {
            self.stop_session(dir);
        }
        if !was_on && !managed && on_stop.is_none() {
            return writeln!(out, "server mode was already off").map_err(&w);
        }
        writeln!(out, "server mode off — the Mac can sleep again").map_err(&w)?;
        if managed {
            match self.host.serve_stop() {
                Ok(_) => writeln!(out, "  beagle serve stopped").map_err(&w)?,
                Err(e) => writeln!(out, "  ✗ beagle serve: couldn't stop: {e}").map_err(&w)?,
            }
        }
        let mut after: Vec<Step> = on_stop
            .map(|f| Step::for_file("on-stop", Path::new(&f)))
            .into_iter()
            .collect();
        match self.load_config() {
            Ok(config) => after.extend(config.after),
            Err(e) => writeln!(out, "  ✗ {e}").map_err(&w)?,
        }
        for step in &after {
            let line = match steps::run(&self.progs.sh, step, &self.state.path(LOG_FILE)) {
                Ok(()) => Line::ok(format!("step {}", step.name)),
                Err(e) => Line::fail(format!("step {}: {e}", step.name)),
            };
            writeln!(out, "  {line}").map_err(&w)?;
        }
        Ok(())
    }

    /// Stops a session's processes and clears its files.
    pub(crate) fn stop_session(&self, dir: &StateDir) {
        // The timer execs into `off`, so its pid can be this process.
        if let Some(tp) = dir.read_pid("timer.pid") {
            if tp != std::process::id() {
                stop(&self.progs, Target::Pid(tp));
            }
        }
        if let Some(sp) = dir.read_pid("startup.pid") {
            stop(&self.progs, Target::Group(sp));
        }
        if let Some(cp) = dir.read_pid("caffeinate.pid") {
            stop(&self.progs, Target::Pid(cp));
        }
        dir.clear();
    }

    /// The checklist: `beagle serve` (when server mode runs it), its
    /// certificate, agentd (started when it is loaded but down), exposure.
    pub(crate) fn checks(&self, serve: bool) -> Vec<Line> {
        let host = &self.host;
        let mut lines = Vec::new();
        if serve {
            let url = host.serve_url().unwrap_or_else(|| "?".into());
            lines.push(match (host.serve(), host.serve_health()) {
                (Ok(Svc::Running), Ok(())) => Line::ok(format!("beagle serve: answering at {url}")),
                (Ok(Svc::Running), Err(e)) => Line::fail(format!(
                    "beagle serve: running but {url}/v1/health fails ({e}) — see `beagle serve status`"
                )),
                (Ok(_), _) => Line::fail("beagle serve: stopped — `beagle server-mode on` starts it"),
                (Err(e), _) => Line::fail(format!("beagle serve: {e}")),
            });
            lines.push(match host.cert_days_left() {
                Ok(d) if d < 0 => {
                    Line::fail("certificate: expired — renew it, then `beagle serve install`")
                }
                Ok(d) if d < 14 => Line::warn(format!(
                    "certificate: {d} days left — renew it, then `beagle serve install`"
                )),
                Ok(d) => Line::ok(format!("certificate: {d} days left")),
                Err(e) => Line::fail(format!("certificate: {e}")),
            });
        }
        lines.push(crate::report::agentd_line(host.as_ref()));
        lines.extend(crate::report::exposure_lines(&host.exposure()));
        lines
    }
}

/// The file as an absolute path; it must be a regular file.
fn absolute_file(file: &Path) -> Result<PathBuf, String> {
    if !file.is_file() {
        return Err(format!("no such file: {}", file.display()));
    }
    std::path::absolute(file).map_err(|e| format!("no such file: {}: {e}", file.display()))
}
