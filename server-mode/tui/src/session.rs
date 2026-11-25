//! The commands: `on`, `off`, `status`, `log` and `lock`.

use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::cli::{Cmd, OnOpts};
use crate::procs::{alive, output, spawn_detached, stop, Programs, Target};
use crate::state::{fmt_dur, tail, StateDir, LOG_FILE};

/// What the timer runs: sleep, then become `<beagle> server-mode off`
/// (`exec`, so `off` sees its own pid in `timer.pid` and spares itself).
const TIMER_SCRIPT: &str = r#"sleep "$1"; exec "$2" server-mode off"#;

/// Ctrl-Cmd-Q, the Apple menu → Lock Screen shortcut. Needs Accessibility access
/// for the terminal; without it `lock` falls back to display sleep.
const LOCK_SCRIPT: &str =
    r#"tell application "System Events" to keystroke "q" using {control down, command down}"#;

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
}

type Res = Result<(), String>;

fn io_err(what: &str) -> impl Fn(io::Error) -> String + '_ {
    move |e| format!("{what}: {e}")
}

fn now() -> i64 {
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
        Ok(Self {
            state: StateDir::new(state),
            legacy: legacy.map(StateDir::new),
            progs: Programs::system(),
            exe: beagle_utils::paths::stable_exe(&exe),
        })
    }

    pub fn exec(&self, cmd: Cmd, out: &mut dyn Write) -> Res {
        let res = match cmd {
            Cmd::On(opts) => self.on(&opts, out),
            Cmd::Off => self.off(false, out),
            Cmd::Status => self.status(out),
            Cmd::Log => self.log(out),
            Cmd::Lock => self.lock_screen(out),
            Cmd::Help => Ok(()),
        };
        res.and_then(|()| out.flush().map_err(io_err("write")))
    }

    /// This tool's dir, then the old script's.
    fn dirs(&self) -> impl Iterator<Item = &StateDir> {
        std::iter::once(&self.state).chain(self.legacy.as_ref())
    }

    fn is_on(&self, dir: &StateDir) -> bool {
        dir.read_pid("caffeinate.pid")
            .is_some_and(|p| alive(&self.progs, Target::Pid(p)))
    }

    /// The dir holding the running session, if any.
    fn active(&self) -> Option<&StateDir> {
        self.dirs().find(|d| self.is_on(d))
    }

    fn on(&self, opts: &OnOpts, out: &mut dyn Write) -> Res {
        if self.active().is_some() {
            return Err("already on — run 'beagle server-mode off' first \
                        (or 'beagle server-mode status')"
                .into());
        }
        let file = opts.file.as_deref().map(absolute_file).transpose()?;
        self.off(true, out)?; // clear stale state from a crash or reboot

        let state = &self.state;
        let start = now();
        state.write("since", start).map_err(io_err("write state"))?;

        // -t is a backstop: the timer also stops the startup file, but if
        // it dies caffeinate still lets the Mac sleep on schedule.
        let mut caf = self.progs.nohup(&self.progs.caffeinate);
        caf.args(["-i", "-s"]);
        if let Some(secs) = opts.secs {
            caf.args(["-t", &secs.to_string()]);
        }
        caf.stdout(Stdio::null()).stderr(Stdio::null());
        let pid = spawn_detached(&mut caf).map_err(io_err("couldn't start caffeinate"))?;
        state
            .write("caffeinate.pid", pid)
            .map_err(io_err("write state"))?;

        if let Some(secs) = opts.secs {
            let until = start.saturating_add(i64::try_from(secs).unwrap_or(i64::MAX));
            state.write("until", until).map_err(io_err("write state"))?;
            let mut timer = self.progs.nohup(&self.progs.sh);
            timer
                .args(["-c", TIMER_SCRIPT, "_", &secs.to_string()])
                .arg(&self.exe)
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            let pid = spawn_detached(&mut timer).map_err(io_err("couldn't start the timer"))?;
            state
                .write("timer.pid", pid)
                .map_err(io_err("write state"))?;
        }

        if let Some(file) = &file {
            self.start_file(file)?;
        }

        let until = match &opts.hours {
            Some(h) => format!(" for {h}h"),
            None => " until 'beagle server-mode off'".into(),
        };
        let w = io_err("write");
        writeln!(out, "server mode on — awake{until}").map_err(&w)?;
        if let Some(file) = &file {
            writeln!(
                out,
                "  running {} (log: beagle server-mode log)",
                file.display()
            )
            .map_err(&w)?;
        }
        if self.on_battery() {
            writeln!(
                out,
                "  on battery: it stays awake, but plug in for anything long"
            )
            .map_err(&w)?;
        }
        if opts.lock {
            self.lock_screen(out)?;
        }
        Ok(())
    }

    /// Runs the startup file in its own process group, output to the log.
    /// Executable files run as-is (their shebang wins), others with bash.
    fn start_file(&self, file: &Path) -> Res {
        let state = &self.state;
        state
            .write("file", file.display())
            .map_err(io_err("write state"))?;
        let log = fs::File::create(state.path(LOG_FILE)).map_err(io_err("create the log"))?;
        let err = log.try_clone().map_err(io_err("open the log"))?;
        let executable = fs::metadata(file).is_ok_and(|m| m.permissions().mode() & 0o111 != 0);
        let mut cmd = if executable {
            self.progs.nohup(file)
        } else {
            let mut c = self.progs.nohup(&self.progs.bash);
            c.arg(file);
            c
        };
        cmd.stdout(log).stderr(err);
        let pid = spawn_detached(&mut cmd)
            .map_err(|e| format!("couldn't start {}: {e}", file.display()))?;
        state
            .write("startup.pid", pid)
            .map_err(io_err("write state"))
    }

    fn off(&self, quiet: bool, out: &mut dyn Write) -> Res {
        let was_on = self.active().is_some();
        for dir in self.dirs() {
            self.stop_session(dir);
        }
        if quiet {
            return Ok(());
        }
        let msg = if was_on {
            "server mode off — the Mac can sleep again"
        } else {
            "server mode was already off"
        };
        writeln!(out, "{msg}").map_err(io_err("write"))
    }

    fn stop_session(&self, dir: &StateDir) {
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

    fn status(&self, out: &mut dyn Write) -> Res {
        let w = io_err("write");
        let Some(dir) = self.active() else {
            return writeln!(out, "server mode: off").map_err(&w);
        };
        let now = now();
        let since = dir.read_i64("since").unwrap_or(now);
        writeln!(out, "server mode: on (for {})", fmt_dur(now - since)).map_err(&w)?;
        if let Some(until) = dir.read_i64("until") {
            let clock = self
                .clock(until)
                .map(|c| format!(" ({c})"))
                .unwrap_or_default();
            writeln!(out, "  ends in {}{clock}", fmt_dur(until - now)).map_err(&w)?;
        } else {
            writeln!(out, "  no end time — runs until 'beagle server-mode off'").map_err(&w)?;
        }
        if let Some(file) = dir.read("file") {
            match dir.read_pid("startup.pid") {
                Some(sp) if alive(&self.progs, Target::Pid(sp)) => {
                    writeln!(out, "  startup file: {file} (running, pid {sp})").map_err(&w)?;
                }
                _ => writeln!(
                    out,
                    "  startup file: {file} (exited — see 'beagle server-mode log')"
                )
                .map_err(&w)?,
            }
        }
        Ok(())
    }

    /// `Tue 18:30` for an epoch second, in local time (`date -r`).
    fn clock(&self, epoch: i64) -> Option<String> {
        let at = epoch.to_string();
        let ok = |args: &[&str]| {
            output(&self.progs.date, args)
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
                .filter(|s| !s.is_empty())
        };
        ok(&["-r", &at, "+%a %H:%M"]).or_else(|| ok(&["-d", &format!("@{at}"), "+%a %H:%M"]))
    }

    fn log(&self, out: &mut dyn Write) -> Res {
        let dir = self
            .active()
            .filter(|d| d.has_log())
            .or_else(|| self.dirs().find(|d| d.has_log()))
            .ok_or("no startup log yet")?;
        let bytes = fs::read(dir.path(LOG_FILE)).map_err(io_err("read the log"))?;
        out.write_all(tail(&String::from_utf8_lossy(&bytes), 50).as_bytes())
            .map_err(io_err("write"))
    }

    fn on_battery(&self) -> bool {
        output(&self.progs.pmset, &["-g", "batt"])
            .is_some_and(|o| String::from_utf8_lossy(&o.stdout).contains("Battery Power"))
    }

    /// Lock for real. `pmset displaysleepnow` alone only darkens the
    /// display and asks for the password only after the screen-lock delay,
    /// so it is the fallback, not the way.
    fn lock_screen(&self, out: &mut dyn Write) -> Res {
        let w = io_err("write");
        let locked =
            output(&self.progs.osascript, &["-e", LOCK_SCRIPT]).is_some_and(|o| o.status.success());
        if locked {
            return writeln!(out, "  screen locked").map_err(&w);
        }
        let delay = output(&self.progs.sysadminctl, &["-screenLock", "status"])
            .and_then(|o| {
                let text = format!(
                    "{}{}",
                    String::from_utf8_lossy(&o.stdout),
                    String::from_utf8_lossy(&o.stderr)
                );
                lock_delay(&text)
            })
            .unwrap_or_else(|| "the lock delay".into());
        writeln!(
            out,
            "  couldn't lock: give your terminal Accessibility access\n  \
             (System Settings → Privacy & Security → Accessibility), then 'beagle server-mode lock'\n  \
             sleeping the display instead — it only locks after {delay}"
        )
        .map_err(&w)?;
        output(&self.progs.pmset, &["displaysleepnow"]);
        Ok(())
    }
}

/// The file as an absolute path; it must be a regular file.
fn absolute_file(file: &Path) -> Result<PathBuf, String> {
    if !file.is_file() {
        return Err(format!("no such file: {}", file.display()));
    }
    std::path::absolute(file).map_err(|e| format!("no such file: {}: {e}", file.display()))
}

/// The screen-lock delay in `sysadminctl -screenLock status` output: the
/// first of `<n> seconds`, `immediate` or `off`.
pub(crate) fn lock_delay(text: &str) -> Option<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    words.iter().enumerate().find_map(|(i, w)| match *w {
        "immediate" | "off" => Some((*w).to_owned()),
        n if n.chars().all(|c| c.is_ascii_digit())
            && words.get(i + 1).is_some_and(|s| s.starts_with("seconds")) =>
        {
            Some(format!("{n} seconds"))
        }
        _ => None,
    })
}
