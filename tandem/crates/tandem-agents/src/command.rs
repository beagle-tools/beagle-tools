//! Build the argv for each runner kind.
//!
//! Flags deliberately mirror agentd (`workflows/` in this monorepo) so behaviour matches
//! what already works there:
//! - claude: `claude -p --output-format stream-json --verbose`
//! - codex:  `codex exec --json --skip-git-repo-check -` (prompt on stdin)
//! - grok:   `grok --output-format streaming-json -p <prompt>` (Grok Build
//!   takes the prompt as an argument, not stdin)

use tandem_core::agent::{AgentSpec, RunnerKind};

/// A fully resolved program + argument list. The prompt is fed via
/// stdin unless `prompt_in_argv` is set, in which case the runner
/// appends it as the final argument (Grok Build has no stdin mode).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunnerCommand {
    /// Executable to spawn — a bare name resolved on the augmented `PATH`, or
    /// `/bin/sh` for custom runners.
    pub program: String,
    /// Arguments, excluding the prompt.
    pub args: Vec<String>,
    /// True when the prompt must be appended as the last argument instead of
    /// written to stdin.
    pub prompt_in_argv: bool,
}

/// Translate a spec's [`RunnerKind`] (plus model, tool allowlist, and
/// extra system prompt) into the exact command line to spawn.
pub fn build_command(spec: &AgentSpec) -> RunnerCommand {
    match &spec.runner {
        RunnerKind::ClaudeHeadless => claude_command(spec),
        RunnerKind::CodexHeadless => codex_command(spec),
        RunnerKind::GrokHeadless => grok_command(spec),
        RunnerKind::Custom { command } => custom_command(command),
    }
}

fn claude_command(spec: &AgentSpec) -> RunnerCommand {
    let mut args = vec![
        "-p".to_owned(),
        "--output-format".to_owned(),
        "stream-json".to_owned(),
        "--verbose".to_owned(),
    ];
    if let Some(model) = &spec.model {
        args.push("--model".to_owned());
        args.push(model.clone());
    }
    if !spec.allowed_tools.is_empty() {
        args.push("--allowedTools".to_owned());
        args.push(spec.allowed_tools.join(","));
    }
    if let Some(extra) = &spec.append_system_prompt {
        args.push("--append-system-prompt".to_owned());
        args.push(extra.clone());
    }
    RunnerCommand {
        program: "claude".to_owned(),
        args,
        prompt_in_argv: false,
    }
}

fn codex_command(spec: &AgentSpec) -> RunnerCommand {
    let mut args = vec![
        "exec".to_owned(),
        "--json".to_owned(),
        "--skip-git-repo-check".to_owned(),
    ];
    if let Some(model) = &spec.model {
        args.push("-m".to_owned());
        args.push(model.clone());
    }
    args.push("-".to_owned());
    RunnerCommand {
        program: "codex".to_owned(),
        args,
        prompt_in_argv: false,
    }
}

/// Grok Build: headless single-prompt mode. `--always-approve` because
/// headless has no approval UI (same v1 trust model as claude's
/// --allowedTools; the v2 sandbox is the real boundary). The prompt is
/// appended after `-p` by the runner (`prompt_in_argv`).
fn grok_command(spec: &AgentSpec) -> RunnerCommand {
    let mut args = vec![
        "--output-format".to_owned(),
        "streaming-json".to_owned(),
        "--no-auto-update".to_owned(),
        "--always-approve".to_owned(),
    ];
    if let Some(model) = &spec.model {
        args.push("-m".to_owned());
        args.push(model.clone());
    }
    args.push("-p".to_owned());
    RunnerCommand {
        program: "grok".to_owned(),
        args,
        prompt_in_argv: true,
    }
}

/// Custom commands run through the shell so users can write pipelines.
fn custom_command(command: &str) -> RunnerCommand {
    RunnerCommand {
        program: "/bin/sh".to_owned(),
        args: vec!["-c".to_owned(), command.to_owned()],
        prompt_in_argv: false,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use tandem_core::agent::AuthMode;

    fn spec(runner: RunnerKind) -> AgentSpec {
        AgentSpec {
            name: "test".into(),
            runner,
            auth: AuthMode::CliSession,
            model: Some("opus".into()),
            allowed_tools: vec!["Read".into(), "Grep".into()],
            append_system_prompt: None,
            prompt: "review this".into(),
            env: BTreeMap::new(),
            network_allowlist: vec![],
            timeout_minutes: 15,
            summary: true,
        }
    }

    #[test]
    fn claude_flags_match_agentd() {
        let cmd = build_command(&spec(RunnerKind::ClaudeHeadless));
        assert_eq!(cmd.program, "claude");
        assert_eq!(
            cmd.args,
            vec![
                "-p",
                "--output-format",
                "stream-json",
                "--verbose",
                "--model",
                "opus",
                "--allowedTools",
                "Read,Grep"
            ]
        );
    }

    #[test]
    fn codex_reads_prompt_from_stdin() {
        let cmd = build_command(&spec(RunnerKind::CodexHeadless));
        assert_eq!(cmd.program, "codex");
        assert_eq!(cmd.args.last().unwrap(), "-");
        assert!(cmd.args.contains(&"--skip-git-repo-check".to_owned()));
    }

    #[test]
    fn grok_takes_prompt_as_final_arg() {
        let cmd = build_command(&spec(RunnerKind::GrokHeadless));
        assert_eq!(cmd.program, "grok");
        assert!(cmd.prompt_in_argv);
        assert_eq!(cmd.args.last().unwrap(), "-p");
        assert!(cmd.args.contains(&"--no-auto-update".to_owned()));
        assert_eq!(cmd.args[cmd.args.len() - 3], "-m");
        assert_eq!(cmd.args[cmd.args.len() - 2], "opus");
    }

    #[test]
    fn custom_runs_via_shell() {
        let cmd = build_command(&spec(RunnerKind::Custom {
            command: "my-agent | tee log".into(),
        }));
        assert_eq!(cmd.program, "/bin/sh");
        assert_eq!(cmd.args, vec!["-c", "my-agent | tee log"]);
    }
}
