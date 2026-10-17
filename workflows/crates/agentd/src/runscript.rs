//! Turning an `AgentSpec` into the shell script a run executes: the
//! runner's argv, the prompt files it is fed, and the run context the
//! daemon writes beside them.
//!
//! Split from `spawner.rs`, which owns the lifecycle — picking up a
//! pending run, making its tmux session, recording state. This owns
//! only the string that session runs.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use agent_core::paths;
#[cfg(test)]
use agent_core::types::Isolation;
use agent_core::types::{AgentSpec, Run, Runner};
use anyhow::{bail, Context, Result};

use crate::runenv::{env_exports, run_context};
use beagle_utils::shell::quote as shell_quote;

pub(crate) fn build_run_script(
    spec: &AgentSpec,
    run: &Run,
    log: &Path,
    root: &str,
) -> Result<String> {
    let data_dir = paths::data_dir()?.join(&run.id);
    std::fs::create_dir_all(&data_dir)
        .with_context(|| format!("create data dir {}", data_dir.display()))?;

    let body = match spec.runner {
        Runner::Script => {
            let command = spec
                .command
                .as_deref()
                .context("script runner has no command")?;
            script_body(command, log)
        }
        Runner::Claude | Runner::Codex => {
            let prompt = spec.prompt.as_deref().context("agent has no prompt")?;
            let mut sources = prompt_sources(prompt)?;
            // LLM runs get their params INSIDE the prompt as literal values:
            // scoped tool rules (e.g. `Bash(notes:*)`) deny commands that
            // contain `$VAR` expansions, so env-var params are unreadable to
            // a locked-down headless session.
            let context = data_dir.join("90-run-context.md");
            std::fs::write(&context, run_context(run, &data_dir, root))
                .context("write run context")?;
            sources.push(context);
            // `agentctl run --model` wins over the manifest's `model:`.
            let model = run.model.as_deref().or(spec.model.as_deref());
            run_script(spec, model, &sources, log)
        }
    };
    Ok(format!(
        "{}{body}",
        env_exports(spec, run, &data_dir, root)?
    ))
}

/// The runner CLI invocation, with the model flag quoted when present. The
/// prompt is fed via stdin redirection (codex needs the explicit `-` arg).
/// `model` is already resolved (run override, else the manifest's).
fn runner_command(spec: &AgentSpec, model: Option<&str>) -> String {
    // Script never reaches here (build_run_script branches first); the
    // match stays total via the Codex arm.
    // The workdir is the manifest's deliberate choice, so codex's "trusted
    // (git) directory" refusal is skipped — without it any non-repo workdir
    // (e.g. ~) fails instantly.
    let mut cmd = match spec.runner {
        Runner::Claude => "claude -p --output-format stream-json --verbose".to_owned(),
        Runner::Codex | Runner::Script => "codex exec --json --skip-git-repo-check".to_owned(),
    };
    if let Some(model) = model {
        let flag = match spec.runner {
            Runner::Claude => "--model",
            Runner::Codex | Runner::Script => "-m",
        };
        let _ = write!(cmd, " {flag} {}", shell_quote(model));
    }
    if spec.runner == Runner::Claude {
        if let Some(tools) = spec.allowed_tools.as_deref().filter(|t| !t.is_empty()) {
            let _ = write!(cmd, " --allowedTools {}", shell_quote(&tools.join(",")));
        }
        // Deny rules take precedence over --allowedTools, which is the only
        // way to allow a command family minus its dangerous members.
        if let Some(tools) = spec.disallowed_tools.as_deref().filter(|t| !t.is_empty()) {
            let _ = write!(cmd, " --disallowedTools {}", shell_quote(&tools.join(",")));
        }
        if let Some(file) = &spec.append_system_prompt {
            // The file is read at run time by the shell, so edits apply to
            // the next run — same live semantics as directory prompts.
            let _ = write!(
                cmd,
                " --append-system-prompt \"$(cat {})\"",
                shell_quote(&file.to_string_lossy())
            );
        }
        for dir in &spec.add_dirs {
            let _ = write!(cmd, " --add-dir {}", shell_quote(&dir.to_string_lossy()));
        }
        // Pin the MCP surface: --strict-mcp-config makes this file the ONLY
        // source of servers, so the ambient claude.ai account connectors are
        // not even connected to. Without it a locked-down agent still reaches
        // for every connector on the account.
        if let Some(file) = &spec.mcp_config {
            let _ = write!(
                cmd,
                " --mcp-config {} --strict-mcp-config",
                shell_quote(&file.to_string_lossy())
            );
        }
    }
    if spec.runner == Runner::Codex {
        cmd.push_str(" -");
    }
    cmd
}

/// The script-runner body: the manifest's command, logged like any run. A
/// subshell (not a brace group) contains the command, so an explicit `exit`
/// inside it still reaches the exit-marker line with the right code.
fn script_body(command: &str, log: &Path) -> String {
    let log = shell_quote(&log.to_string_lossy());
    format!("( {command} ) >> {log} 2>&1; printf \"__AGENTD_EXIT__:%s\\n\" \"$?\" >> {log}")
}

/// The prompt files fed to the runner: the file itself, or — when the
/// manifest points at a directory — every `.md` inside it, sorted by name so
/// numbered prefixes (`00-persona.md`, `10-task.md`) control the order.
/// Resolved at spawn time, so edits apply to the next run without a reapply.
fn prompt_sources(prompt: &Path) -> Result<Vec<PathBuf>> {
    if !prompt.is_dir() {
        return Ok(vec![prompt.to_path_buf()]);
    }
    let entries = std::fs::read_dir(prompt)
        .with_context(|| format!("read prompt directory {}", prompt.display()))?;
    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "md"))
        .collect();
    files.sort();
    if files.is_empty() {
        bail!("prompt directory {} has no .md files", prompt.display());
    }
    Ok(files)
}

/// The full shell script run inside the tmux session: runner reading the
/// prompt (one file redirected, several concatenated), appending
/// stdout+stderr to the log, then an exit-code marker line.
fn run_script(spec: &AgentSpec, model: Option<&str>, prompts: &[PathBuf], log: &Path) -> String {
    let log = shell_quote(&log.to_string_lossy());
    let quoted: Vec<String> = prompts
        .iter()
        .map(|p| shell_quote(&p.to_string_lossy()))
        .collect();
    let feed = if quoted.len() == 1 {
        format!("{} < {}", runner_command(spec, model), quoted[0])
    } else {
        format!("cat {} | {}", quoted.join(" "), runner_command(spec, model))
    };
    format!("{feed} >> {log} 2>&1; printf \"__AGENTD_EXIT__:%s\\n\" \"$?\" >> {log}")
}

#[cfg(test)]
// Tests should fail loudly on a broken fixture; unwrapping is the clearest way.
#[allow(clippy::unwrap_used)]
#[path = "tests/runscript.rs"]
mod tests;
