//! `beagle handoff`: pipe a reviewed RCA to the configured external agent.

use std::fs;
use std::path::PathBuf;

use beagle::model::RcaId;
use beagle::store::Store;
use beagle::Error;
use beagle_utils::paths::expand_tilde;

use crate::{effective_config, effective_root};

/// `beagle handoff <slug>`: launch the configured agent on a reviewed RCA.
/// The composed prompt (config prompt file + the RCA write-up) is piped to
/// the agent's stdin; it runs in the store root with the RCA's identity in
/// the environment, and its output streams to the terminal. Book-ended in
/// the workspace log so the live view shows the hand-off.
pub(crate) fn run_handoff(root: Option<PathBuf>, id: &RcaId) -> Result<(), Error> {
    let handoff = effective_config()?
        .and_then(|c| c.handoff)
        .filter(|h| !h.command.is_empty())
        .ok_or_else(|| Error::Tool {
            tool: "handoff",
            message: "no agent configured — add a [handoff] section with a \
                      `command` to your config (`beagle config`)"
                .to_owned(),
        })?;

    let store = Store::open(&effective_root(root)?)?;
    store.read_meta(id)?; // proves the workspace exists

    let prompt = match handoff.prompt {
        Some(path) => {
            let path = expand_tilde(&path);
            Some(fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?)
        }
        None => None,
    };
    let write_up = store.export_markdown(id)?;
    let input = beagle::handoff::compose_input(id.as_str(), prompt.as_deref(), &write_up);

    let workspace = store.workspace_dir(id);
    // `command` is non-empty (filtered above), so `split_first` is `Some`.
    let Some((program, args)) = handoff.command.split_first() else {
        return Ok(());
    };
    println!("→ handing {id} to `{program}` …");
    let _ = store.append_log(id, &format!("agent hand-off started (`{program}`)"));

    let status = run_agent(program, args, store.root(), id, &workspace, &input);
    match status {
        Ok(status) if status.success() => {
            let _ = store.append_log(id, "agent hand-off finished");
            println!("✓ hand-off complete");
            Ok(())
        }
        Ok(status) => {
            let _ = store.append_log(id, &format!("agent hand-off exited with {status}"));
            Err(Error::Tool {
                tool: "handoff",
                message: format!("`{program}` exited with {status}"),
            })
        }
        Err(e) => Err(Error::Tool {
            tool: "handoff",
            message: format!("could not launch `{program}`: {e}"),
        }),
    }
}

/// Spawns the hand-off agent: stdin fed the composed prompt, stdout/stderr
/// inherited so its work streams live, run in the store root with the RCA
/// in the environment.
fn run_agent(
    program: &str,
    args: &[String],
    root: &std::path::Path,
    id: &RcaId,
    workspace: &std::path::Path,
    input: &str,
) -> std::io::Result<std::process::ExitStatus> {
    use std::io::Write as _;
    use std::process::{Command as Proc, Stdio};

    let mut child = Proc::new(program)
        .args(args)
        .current_dir(root)
        .env("BEAGLE_RCA_SLUG", id.as_str())
        .env("BEAGLE_RCA_DIR", workspace)
        .env("BEAGLE_RCA_ROOT", root)
        .stdin(Stdio::piped())
        .spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(input.as_bytes())?;
    } // drop closes stdin so the agent sees EOF
    child.wait()
}
