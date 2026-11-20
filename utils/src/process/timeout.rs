//! Bounded child processes: [`Command::output`] with a deadline.

use std::io::{self, Read, Write as _};
use std::process::{Child, Command, Output, Stdio};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// Longest sleep between `try_wait` polls. Polling starts at 1 ms and backs
/// off to this, so a fast command returns promptly and a slow one costs ~20
/// wakeups a second.
const MAX_POLL: Duration = Duration::from_millis(50);

/// How long to wait for a pipe's EOF once the child has exited. Everything
/// the child wrote is already in the pipe buffer, so this only matters when
/// a grandchild inherited the pipe and keeps it open.
const EOF_GRACE: Duration = Duration::from_millis(500);

/// Runs `cmd` like [`Command::output`], but gives up after `timeout`.
///
/// Stdin is null; stdout and stderr are captured. Both pipes are drained on
/// helper threads while the child runs, so a chatty child can never
/// deadlock on a full pipe. On expiry the child is killed and reaped (no
/// zombie) and the error has kind [`io::ErrorKind::TimedOut`] with a
/// message naming the command, e.g. ``"`gh pr view` timed out after 20s"``.
///
/// Only the direct child is killed and waited for. Should a grandchild keep
/// the output pipes open (a daemonizing `xclip`), the call still returns
/// with what the child wrote; the reader threads are left detached.
pub fn output_with_timeout(cmd: &mut Command, timeout: Duration) -> io::Result<Output> {
    run(cmd, None, timeout)
}

/// [`output_with_timeout`], but with `input` written to the child's stdin
/// (from a helper thread, then closed) instead of a null stdin.
pub fn output_with_input_and_timeout(
    cmd: &mut Command,
    input: &[u8],
    timeout: Duration,
) -> io::Result<Output> {
    run(cmd, Some(input.to_vec()), timeout)
}

fn run(cmd: &mut Command, input: Option<Vec<u8>>, timeout: Duration) -> io::Result<Output> {
    let label = label(cmd);
    let deadline = Instant::now() + timeout;
    cmd.stdin(if input.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    })
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
    let mut child = cmd.spawn()?;

    if let (Some(bytes), Some(mut stdin)) = (input, child.stdin.take()) {
        // A child that exits without reading gives EPIPE; that is its call.
        thread::spawn(move || {
            let _ = stdin.write_all(&bytes);
        });
    }
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());

    let Some(status) = wait_until(&mut child, deadline)? else {
        // Kill can fail only if the child already exited; wait reaps it
        // either way so no zombie is left behind.
        let _ = child.kill();
        let _ = child.wait();
        return Err(timed_out(&label, timeout));
    };
    Ok(Output {
        status,
        stdout: collect(&stdout),
        stderr: collect(&stderr),
    })
}

/// Polls `child` until it exits (`Some(status)`) or `deadline` passes
/// (`None`).
fn wait_until(
    child: &mut Child,
    deadline: Instant,
) -> io::Result<Option<std::process::ExitStatus>> {
    let mut poll = Duration::from_millis(1);
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }
        let now = Instant::now();
        if now >= deadline {
            return Ok(None);
        }
        thread::sleep(poll.min(deadline - now));
        poll = (poll * 2).min(MAX_POLL);
    }
}

/// A pipe being read to EOF on a helper thread.
struct Drain {
    /// Bytes read so far, appended chunk by chunk.
    buf: Arc<Mutex<Vec<u8>>>,
    /// Fires (or disconnects) once the reader hit EOF or an error.
    done: mpsc::Receiver<()>,
}

/// Starts reading `pipe` on a helper thread.
fn drain<R: Read + Send + 'static>(pipe: Option<R>) -> Drain {
    let buf = Arc::new(Mutex::new(Vec::new()));
    let (tx, done) = mpsc::channel();
    if let Some(mut pipe) = pipe {
        let sink = Arc::clone(&buf);
        thread::spawn(move || {
            let mut chunk = [0u8; 8192];
            // EOF, or a read error mid-stream: keep what was read so far.
            while let Ok(n @ 1..) = pipe.read(&mut chunk) {
                if let Ok(mut out) = sink.lock() {
                    out.extend_from_slice(&chunk[..n]);
                }
            }
            let _ = tx.send(());
        });
    }
    Drain { buf, done }
}

/// The bytes read from an exited child's pipe. Waits briefly for EOF; if a
/// grandchild (a daemonizing `xclip`, an ssh control master) still holds
/// the pipe, returns what the child wrote and leaves the reader detached.
fn collect(drain: &Drain) -> Vec<u8> {
    let _ = drain.done.recv_timeout(EOF_GRACE);
    drain
        .buf
        .lock()
        .map(|mut b| std::mem::take(&mut *b))
        .unwrap_or_default()
}

/// A short name for `cmd` in error messages: the program's file name plus
/// its leading non-flag arguments (at most two), e.g. `gh pr view`. A
/// leading `-C <dir>` (git's working-directory flag) is skipped.
fn label(cmd: &Command) -> String {
    let program = cmd.get_program().to_string_lossy();
    let program = program.rsplit('/').next().unwrap_or(&program).to_owned();
    let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();
    let mut rest = args.as_slice();
    while let [flag, _dir, tail @ ..] = rest {
        if flag != "-C" {
            break;
        }
        rest = tail;
    }
    rest.iter()
        .take_while(|a| !a.starts_with('-'))
        .take(2)
        .fold(program, |acc, arg| format!("{acc} {arg}"))
}

fn timed_out(label: &str, timeout: Duration) -> io::Error {
    io::Error::new(
        io::ErrorKind::TimedOut,
        format!("`{label}` timed out after {}", human(timeout)),
    )
}

/// `20s` for whole seconds, `200ms` otherwise.
fn human(d: Duration) -> String {
    if d.subsec_millis() == 0 && d.as_secs() > 0 {
        format!("{}s", d.as_secs())
    } else {
        format!("{}ms", d.as_millis())
    }
}

#[cfg(all(test, unix))]
// Tests fail loudly on setup errors; unwrap is the clearest way to do that.
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn fast_command_returns_its_output() {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "printf out; printf err >&2; exit 3"]);
        let out = output_with_timeout(&mut cmd, Duration::from_secs(10)).unwrap();
        assert_eq!(out.stdout, b"out");
        assert_eq!(out.stderr, b"err");
        assert_eq!(out.status.code(), Some(3));
    }

    #[test]
    fn slow_command_times_out_quickly_and_is_reaped() {
        let dir = tempfile::tempdir().unwrap();
        let pid_file = dir.path().join("pid");
        let mut cmd = Command::new("sh");
        cmd.arg("-c")
            .arg("echo $$ > \"$1\"; exec sleep 5")
            .arg("sh")
            .arg(&pid_file);
        let start = Instant::now();
        let err = output_with_timeout(&mut cmd, Duration::from_millis(200)).unwrap_err();
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "{:?}",
            start.elapsed()
        );
        assert_eq!(err.kind(), io::ErrorKind::TimedOut);
        assert_eq!(err.to_string(), "`sh` timed out after 200ms");

        // The child was killed and reaped: `ps` no longer lists it at all
        // (a zombie would still show up, with state `Z`).
        let pid = std::fs::read_to_string(&pid_file).unwrap();
        let ps = Command::new("ps")
            .args(["-o", "stat=", "-p", pid.trim()])
            .output()
            .unwrap();
        assert!(
            String::from_utf8_lossy(&ps.stdout).trim().is_empty(),
            "child still present: {}",
            String::from_utf8_lossy(&ps.stdout)
        );
    }

    #[test]
    fn large_output_does_not_deadlock() {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "yes | head -c 1000000; yes | head -c 500000 >&2"]);
        let out = output_with_timeout(&mut cmd, Duration::from_secs(20)).unwrap();
        assert!(out.status.success());
        assert_eq!(out.stdout.len(), 1_000_000);
        assert_eq!(out.stderr.len(), 500_000);
    }

    #[test]
    fn grandchild_holding_the_pipe_does_not_block() {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "printf hi; sleep 5 &"]);
        let start = Instant::now();
        let out = output_with_timeout(&mut cmd, Duration::from_secs(10)).unwrap();
        assert!(
            start.elapsed() < Duration::from_secs(3),
            "{:?}",
            start.elapsed()
        );
        assert_eq!(out.stdout, b"hi");
    }

    #[test]
    fn input_is_piped_to_stdin() {
        let mut cmd = Command::new("cat");
        let out =
            output_with_input_and_timeout(&mut cmd, b"hello", Duration::from_secs(10)).unwrap();
        assert_eq!(out.stdout, b"hello");
    }

    #[test]
    fn missing_program_is_a_spawn_error() {
        let mut cmd = Command::new("definitely-not-a-real-program-xyz");
        let err = output_with_timeout(&mut cmd, Duration::from_secs(1)).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn label_names_program_and_subcommands() {
        let mut cmd = Command::new("/opt/homebrew/bin/gh");
        cmd.args(["pr", "view", "--json", "state", "7"]);
        assert_eq!(label(&cmd), "gh pr view");
        let mut cmd = Command::new("git");
        cmd.args(["-C", "/tmp", "push", "-u", "origin"]);
        assert_eq!(label(&cmd), "git push");
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "true"]);
        assert_eq!(label(&cmd), "sh");
    }

    #[test]
    fn human_durations() {
        assert_eq!(human(Duration::from_secs(20)), "20s");
        assert_eq!(human(Duration::from_millis(1500)), "1500ms");
    }
}
