//! The v1 agent runner: a local headless process.
//!
//! Every run gets its own directory containing a JSONL event log (the
//! same format agentd uses) and a `comments.jsonl` drop-box the agent
//! can append comments to. Events stream to the caller over a channel
//! so the UI can tail the run live.

use std::path::PathBuf;
use std::process::Stdio;
use tandem_core::agent::{AgentSpec, RunEvent, RunEventKind, RunStatus};
use tandem_core::{Result, TandemError};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::{mpsc, oneshot};

use crate::command::build_command;
use crate::emitter::Emitter;
use crate::events::{classify_line, extract_embedded_comments, parse_comment};

/// One agent run to start: the spec, its prompt, and where it lives.
#[derive(Debug, Clone)]
pub struct RunRequest {
    /// The agent to run.
    pub spec: AgentSpec,
    /// Fully assembled prompt (see [`crate::context::build_prompt`]).
    pub prompt: String,
    /// Run id (UUID string), echoed on every emitted event.
    pub run_id: String,
    /// Directory owned by this run (log + comments file live here).
    pub run_dir: PathBuf,
    /// Working directory for the process; defaults to `run_dir`.
    pub workdir: Option<PathBuf>,
}

/// Cancels a run when triggered; safe to hold after the run ends.
#[derive(Debug)]
pub struct CancelHandle(Option<oneshot::Sender<()>>);

impl CancelHandle {
    /// Ask the run to stop; the child process is killed.
    pub fn cancel(&mut self) {
        if let Some(tx) = self.0.take() {
            let _ = tx.send(());
        }
    }
}

/// Live handle to a running agent.
#[derive(Debug)]
pub struct RunHandle {
    /// Run id this handle belongs to.
    pub run_id: String,
    /// Classified events (lifecycle, runner output, comments, summaries) as
    /// they happen; closes when the run finishes.
    pub events: mpsc::UnboundedReceiver<RunEvent>,
    /// `events.jsonl` under the run dir — the durable copy of `events`.
    pub log_path: PathBuf,
    /// `comments.jsonl` under the run dir — the agent's drop-box file.
    pub comments_path: PathBuf,
    /// Stops the run; see [`CancelHandle::cancel`].
    pub cancel: CancelHandle,
}

impl RunHandle {
    /// Split into the event stream and the cancel handle so they can live
    /// in different places (event pump task vs. app state).
    pub fn split(self) -> (mpsc::UnboundedReceiver<RunEvent>, CancelHandle) {
        (self.events, self.cancel)
    }
}

/// Runs agents as local child processes (no sandbox yet — see
/// `docs/SANDBOXING.md`).
#[derive(Debug, Clone, Copy, Default)]
pub struct LocalProcessRunner;

impl LocalProcessRunner {
    /// Spawn the agent and return immediately. Output is streamed through
    /// [`RunHandle::events`] and appended to `events.jsonl`; the run is killed
    /// after [`AgentSpec::timeout_minutes`](tandem_core::agent::AgentSpec::timeout_minutes)
    /// or on cancel.
    pub fn start(&self, req: RunRequest) -> Result<RunHandle> {
        std::fs::create_dir_all(&req.run_dir)?;
        let log_path = req.run_dir.join("events.jsonl");
        let comments_path = req.run_dir.join("comments.jsonl");

        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (cancel_tx, cancel_rx) = oneshot::channel();

        let run_id = req.run_id.clone();
        let emitter = Emitter::new(run_id.clone(), &log_path, event_tx)?;
        let comments = comments_path.clone();
        tokio::spawn(drive(req, emitter, cancel_rx, comments));

        Ok(RunHandle {
            run_id,
            events: event_rx,
            log_path,
            comments_path,
            cancel: CancelHandle(Some(cancel_tx)),
        })
    }
}

async fn drive(
    req: RunRequest,
    mut emitter: Emitter,
    cancel_rx: oneshot::Receiver<()>,
    comments_path: PathBuf,
) {
    emitter.lifecycle(RunStatus::Starting, &format!("agent {}", req.spec.name));
    match run_process(&req, &mut emitter, cancel_rx, &comments_path).await {
        Ok(status) => emitter.lifecycle(status, "run finished"),
        Err(e) => emitter.lifecycle(RunStatus::Failed, &e.to_string()),
    }
}

async fn run_process(
    req: &RunRequest,
    emitter: &mut Emitter,
    cancel_rx: oneshot::Receiver<()>,
    comments_path: &PathBuf,
) -> Result<RunStatus> {
    let cmd = build_command(&req.spec);
    let workdir = req.workdir.clone().unwrap_or_else(|| req.run_dir.clone());

    let mut command = Command::new(&cmd.program);
    // Finder-launched apps get the bare system PATH — make sure the
    // runner CLIs installed via Homebrew/npm are findable.
    command.env("PATH", beagle_utils::process::augmented_path());
    command.args(&cmd.args);
    if cmd.prompt_in_argv {
        command.arg(&req.prompt);
    }
    let mut child = command
        .current_dir(&workdir)
        .envs(&req.spec.env)
        .env("TANDEM_RUN_ID", &req.run_id)
        .env("TANDEM_COMMENTS_FILE", comments_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                TandemError::Agent(format!(
                    "can't find `{}` — is it installed? Tandem looks in your PATH \
                     plus /opt/homebrew/bin, /usr/local/bin and ~/.local/bin \
                     (apps launched from Finder don't see your terminal PATH)",
                    cmd.program
                ))
            } else {
                TandemError::Agent(format!("failed to spawn `{}`: {e}", cmd.program))
            }
        })?;

    if let Some(mut stdin) = child.stdin.take() {
        if !cmd.prompt_in_argv {
            // A runner that never reads stdin — or one that finishes
            // before we get here — closes the pipe on us. That's its
            // prerogative, not a failed run, so swallow the EPIPE.
            match stdin.write_all(req.prompt.as_bytes()).await {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => {
                    tracing::debug!(
                        run_id = %req.run_id,
                        "agent closed stdin before the prompt was written"
                    );
                }
                Err(e) => return Err(TandemError::Io(e)),
            }
        }
        drop(stdin);
    }

    // Fan both output streams into one internal channel; a single loop
    // below classifies, logs and forwards them in order of arrival.
    let (line_tx, mut line_rx) = mpsc::unbounded_channel::<String>();
    if let Some(stdout) = child.stdout.take() {
        spawn_line_reader(stdout, line_tx.clone());
    }
    if let Some(stderr) = child.stderr.take() {
        spawn_line_reader(stderr, line_tx.clone());
    }
    drop(line_tx);

    emitter.lifecycle(RunStatus::Running, "process started");

    let timeout = std::time::Duration::from_secs(req.spec.timeout_minutes * 60);
    let deadline = tokio::time::sleep(timeout);
    tokio::pin!(deadline);
    tokio::pin!(cancel_rx);

    let mut outcome: Option<RunStatus> = None;
    // Comments can arrive on several channels (bare stdout line, embedded
    // in claude's assistant/result text, the drop-box file) — dedupe them.
    let mut seen_comments = std::collections::HashSet::new();
    let exit_ok: bool;
    loop {
        tokio::select! {
            line = line_rx.recv() => if let Some(line) = line {
                emit_line(emitter, &mut seen_comments, line);
            } else {
                // Readers closed: process is done (or streams gone) — wait.
                let status = child.wait().await.map_err(TandemError::Io)?;
                exit_ok = status.success();
                break;
            },
            () = &mut deadline, if outcome.is_none() => {
                let _ = child.start_kill();
                outcome = Some(RunStatus::TimedOut);
            }
            _ = &mut cancel_rx, if outcome.is_none() => {
                let _ = child.start_kill();
                outcome = Some(RunStatus::Cancelled);
            }
        }
    }

    // Collect comments the agent appended to its drop-box file (agents
    // whose stdout is structured runner JSON use this path instead).
    if let Ok(contents) = tokio::fs::read_to_string(comments_path).await {
        for comment in contents.lines().filter_map(parse_comment) {
            emit_comment(emitter, &mut seen_comments, &comment);
        }
    }

    Ok(outcome.unwrap_or(if exit_ok {
        RunStatus::Succeeded
    } else {
        RunStatus::Failed
    }))
}

/// Classify one output line, forwarding it plus any comments embedded in
/// structured runner envelopes (claude stream-json wraps model text).
fn emit_line(emitter: &mut Emitter, seen: &mut std::collections::HashSet<String>, line: String) {
    match classify_line(&line) {
        RunEventKind::Comment => {
            if let Some(comment) = parse_comment(&line) {
                emit_comment(emitter, seen, &comment);
            }
        }
        RunEventKind::Runner => {
            let embedded = extract_embedded_comments(&line);
            emitter.emit(RunEventKind::Runner, line);
            for comment in embedded {
                emit_comment(emitter, seen, &comment);
            }
        }
        kind => emitter.emit(kind, line),
    }
}

fn emit_comment(
    emitter: &mut Emitter,
    seen: &mut std::collections::HashSet<String>,
    comment: &crate::events::AgentComment,
) {
    if seen.insert(comment.dedupe_key()) {
        emitter.emit(RunEventKind::Comment, comment.to_payload());
    }
}

fn spawn_line_reader(
    stream: impl tokio::io::AsyncRead + Unpin + Send + 'static,
    tx: mpsc::UnboundedSender<String>,
) {
    tokio::spawn(async move {
        let mut lines = BufReader::new(stream).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use tandem_core::agent::{AuthMode, RunnerKind};

    fn spec_with_command(command: &str) -> AgentSpec {
        AgentSpec {
            name: "echo-agent".into(),
            runner: RunnerKind::Custom {
                command: command.to_owned(),
            },
            auth: AuthMode::CliSession,
            model: None,
            allowed_tools: vec![],
            append_system_prompt: None,
            prompt: String::new(),
            env: BTreeMap::new(),
            network_allowlist: vec![],
            timeout_minutes: 1,
            summary: true,
        }
    }

    async fn collect(handle: &mut RunHandle) -> Vec<RunEvent> {
        let mut events = Vec::new();
        while let Some(e) = handle.events.recv().await {
            events.push(e);
        }
        events
    }

    #[tokio::test]
    async fn custom_agent_emits_comment_from_stdout() {
        let dir = std::env::temp_dir().join(format!("tandem-test-{}", uuid::Uuid::new_v4()));
        let cmd = r#"printf '%s\n' '{"type":"tandem_comment","path":"a.rs","line":1,"body":"hi"}'"#;
        let req = RunRequest {
            spec: spec_with_command(cmd),
            prompt: "ignored".into(),
            run_id: "run-test".into(),
            run_dir: dir.clone(),
            workdir: None,
        };
        let mut handle = LocalProcessRunner.start(req).unwrap();
        let events = collect(&mut handle).await;

        assert!(events.iter().any(|e| e.kind == RunEventKind::Comment));
        let last = events.last().expect("events");
        assert_eq!(last.kind, RunEventKind::Lifecycle);
        assert!(last.payload.contains("succeeded"));
        assert!(dir.join("events.jsonl").exists());
        std::fs::remove_dir_all(dir).ok();
    }

    /// A runner that ignores stdin and exits immediately leaves us
    /// writing into a closed pipe. With a prompt past the ~64KB pipe
    /// buffer the EPIPE is guaranteed rather than a race, and the run
    /// must still be reported on its exit status.
    #[tokio::test]
    async fn prompt_write_to_a_closed_stdin_is_not_a_failure() {
        let dir = std::env::temp_dir().join(format!("tandem-test-{}", uuid::Uuid::new_v4()));
        let req = RunRequest {
            spec: spec_with_command("exit 0"),
            prompt: "x".repeat(512 * 1024),
            run_id: "run-epipe".into(),
            run_dir: dir.clone(),
            workdir: None,
        };
        let mut handle = LocalProcessRunner.start(req).unwrap();
        let events = collect(&mut handle).await;

        let last = events.last().expect("events");
        assert_eq!(last.kind, RunEventKind::Lifecycle);
        assert!(last.payload.contains("succeeded"), "got {}", last.payload);
        std::fs::remove_dir_all(dir).ok();
    }

    #[tokio::test]
    async fn failing_agent_reports_failure() {
        let dir = std::env::temp_dir().join(format!("tandem-test-{}", uuid::Uuid::new_v4()));
        let req = RunRequest {
            spec: spec_with_command("exit 3"),
            prompt: String::new(),
            run_id: "run-fail".into(),
            run_dir: dir.clone(),
            workdir: None,
        };
        let mut handle = LocalProcessRunner.start(req).unwrap();
        let events = collect(&mut handle).await;
        let last = events.last().expect("events");
        assert!(last.payload.contains("failed"));
        std::fs::remove_dir_all(dir).ok();
    }
}
