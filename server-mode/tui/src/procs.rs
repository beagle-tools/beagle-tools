//! Spawning detached children and signalling them by pid or process group.
//!
//! Signals go through the `kill` program (this workspace forbids `unsafe`,
//! so no raw `kill(2)`), each call bounded by a timeout.

use std::io;
use std::os::unix::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use beagle_utils::process::{command, output_with_timeout, which};

/// How long one helper command (`kill`, `pmset`, `date`…) may take.
pub(crate) const HELPER_TIMEOUT: Duration = Duration::from_secs(5);

/// The programs server mode runs. Tests point the system-changing ones
/// (`caffeinate`, `osascript`, `pmset`, `sysadminctl`) at fakes.
#[derive(Debug, Clone)]
pub(crate) struct Programs {
    pub caffeinate: PathBuf,
    pub osascript: PathBuf,
    pub pmset: PathBuf,
    pub sysadminctl: PathBuf,
    pub nohup: PathBuf,
    pub kill: PathBuf,
    pub sh: PathBuf,
    pub bash: PathBuf,
    pub date: PathBuf,
}

impl Programs {
    /// Each program found on the augmented PATH, else by bare name.
    pub fn system() -> Self {
        let find = |name: &str| which(name).unwrap_or_else(|| PathBuf::from(name));
        Self {
            caffeinate: find("caffeinate"),
            osascript: find("osascript"),
            pmset: find("pmset"),
            sysadminctl: find("sysadminctl"),
            nohup: find("nohup"),
            kill: find("kill"),
            sh: find("sh"),
            bash: find("bash"),
            date: find("date"),
        }
    }

    /// `nohup <program> <args…>`: survives the terminal closing.
    pub fn nohup(&self, program: &Path) -> Command {
        let mut cmd = command(&self.nohup);
        cmd.arg(program);
        cmd
    }
}

/// A bounded helper run; `None` when it can't start or times out.
pub(crate) fn output(program: &Path, args: &[&str]) -> Option<std::process::Output> {
    let mut cmd = command(program);
    cmd.args(args);
    output_with_timeout(&mut cmd, HELPER_TIMEOUT).ok()
}

/// What to signal: one process, or a whole process group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Target {
    Pid(u32),
    Group(u32),
}

impl Target {
    fn arg(self) -> String {
        match self {
            Target::Pid(p) => p.to_string(),
            Target::Group(g) => format!("-{g}"),
        }
    }
}

/// Sends `signal` (`-TERM`, `-0`…); true when `kill` succeeded.
fn signal(progs: &Programs, signal: &str, target: Target) -> bool {
    output(&progs.kill, &[signal, "--", &target.arg()]).is_some_and(|o| o.status.success())
}

/// True when `target` exists (`kill -0`).
pub(crate) fn alive(progs: &Programs, target: Target) -> bool {
    signal(progs, "-0", target)
}

/// TERM, then KILL if it is still there after about a second.
pub(crate) fn stop(progs: &Programs, target: Target) {
    if !signal(progs, "-TERM", target) {
        return;
    }
    for _ in 0..5 {
        if !alive(progs, target) {
            return;
        }
        thread::sleep(Duration::from_millis(200));
    }
    signal(progs, "-KILL", target);
}

/// Starts `cmd` as the leader of its own process group, stdin null, and
/// returns its pid. A thread reaps it, so a long-lived caller never keeps
/// a zombie that `kill -0` would still see; when this process exits first
/// the child simply outlives it.
pub(crate) fn spawn_detached(cmd: &mut Command) -> io::Result<u32> {
    let mut child = cmd.process_group(0).stdin(Stdio::null()).spawn()?;
    let pid = child.id();
    thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(pid)
}
