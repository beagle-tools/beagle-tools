//! `agentctl run` / `kill` / `attach` — run lifecycle commands.

use std::os::unix::process::CommandExt;

use agent_core::state::Store;
use agent_core::types::{RunState, Trigger};
use anyhow::{anyhow, bail, Result};

/// Split a `<project>/<agent>` target. `None` when the string is not exactly
/// that shape (no slash, empty halves, or extra slashes).
pub fn parse_qualified(target: &str) -> Option<(&str, &str)> {
    let (project, agent) = target.split_once('/')?;
    if project.is_empty() || agent.is_empty() || agent.contains('/') {
        return None;
    }
    Some((project, agent))
}

/// Create a Pending manual run for `<project>/<agent>`; the daemon picks it
/// up. `parent` links this run to the run that triggered it (DAG lineage);
/// each `params` entry is a `KEY=VALUE` exported as `AGENTD_PARAM_KEY`.
/// `model` overrides the manifest's `model:` for this run only.
pub fn run(
    store: &Store,
    target: &str,
    parent: Option<&str>,
    params: &[String],
    model: Option<&str>,
) -> Result<()> {
    let (project, agent) = parse_qualified(target)
        .ok_or_else(|| anyhow!("expected <project>/<agent>, got {target:?}"))?;
    if store.get_agent(project, agent)?.is_none() {
        bail!("agent {project}/{agent} not found in the store; run `agentctl apply` to sync manifests");
    }
    if let Some(parent_id) = parent {
        if store.get_run(parent_id)?.is_none() {
            bail!("parent run {parent_id} not found");
        }
    }
    let params_json = encode_params(params)?;
    let created = store.create_pending_run(
        project,
        agent,
        Trigger::Manual,
        parent,
        params_json.as_deref(),
        model,
    )?;
    println!("{}", created.id);
    println!("watch with: agentctl logs {} -f", created.id);
    Ok(())
}

/// `KEY=VALUE` pairs → a JSON object string (`None` when no params given).
fn encode_params(params: &[String]) -> Result<Option<String>> {
    if params.is_empty() {
        return Ok(None);
    }
    let mut map = serde_json::Map::new();
    for pair in params {
        let (key, value) = pair
            .split_once('=')
            .ok_or_else(|| anyhow!("--param must be KEY=VALUE, got {pair:?}"))?;
        if key.is_empty() {
            bail!("--param key must not be empty: {pair:?}");
        }
        map.insert(key.to_owned(), serde_json::Value::String(value.to_owned()));
    }
    Ok(Some(serde_json::Value::Object(map).to_string()))
}

/// Kill a Running run: tear down its tmux session and mark it Killed.
pub fn kill(store: &Store, run_id: &str) -> Result<()> {
    let found = store
        .get_run(run_id)?
        .ok_or_else(|| anyhow!("run {run_id} not found"))?;
    if found.state != RunState::Running {
        bail!(
            "run {run_id} is {}, only running runs can be killed",
            found.state
        );
    }
    if let Some(session) = &found.tmux_session {
        // The session may already be gone; the store update below is what counts.
        let _ = agent_core::tmux::kill_session(session);
    }
    store.finish_run(run_id, RunState::Killed, None)?;
    store.insert_event(run_id, "state", Some("killed"))?;
    println!("run {run_id} killed");
    Ok(())
}

/// Replace this process with `tmux attach-session` for a Running run.
pub fn attach(store: &Store, run_id: &str) -> Result<()> {
    let found = store
        .get_run(run_id)?
        .ok_or_else(|| anyhow!("run {run_id} not found"))?;
    if found.state != RunState::Running {
        bail!("cannot attach: run {run_id} is {}", found.state);
    }
    let Some(session) = found.tmux_session else {
        bail!("run {run_id} is running but has no tmux session recorded yet");
    };
    // exec only returns on failure.
    let err = agent_core::tmux::command()
        .args([
            "attach-session",
            "-t",
            &agent_core::tmux::exact_target(&session),
        ])
        .exec();
    Err(anyhow!("failed to exec tmux attach-session: {err}"))
}

#[cfg(test)]
// Tests should fail loudly on a broken fixture; unwrapping is the clearest way.
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use agent_core::types::{AgentSpec, ConcurrencyPolicy, Isolation, Runner};

    fn temp_store(tag: &str) -> Store {
        let path =
            std::env::temp_dir().join(format!("agentctl-run-{tag}-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        Store::open(&path).unwrap()
    }

    fn spec(project: &str, name: &str) -> AgentSpec {
        AgentSpec {
            project: project.to_owned(),
            name: name.to_owned(),
            source: std::path::PathBuf::new(),
            runner: Runner::Claude,
            model: None,
            schedule: None,
            workdir: "/tmp".into(),
            prompt: Some("/tmp/prompt.md".into()),
            command: None,
            allowed_tools: None,
            disallowed_tools: None,
            append_system_prompt: None,
            add_dirs: Vec::new(),
            params: Vec::new(),
            requires_mcp: Vec::new(),
            mcp_config: None,
            env: std::collections::BTreeMap::default(),
            env_file: None,
            database: false,
            max_concurrent: None,
            noop_retention_minutes: None,
            notify: None,
            notify_icon: None,
            notify_icon_failure: None,
            timeout_minutes: 30,
            concurrency: ConcurrencyPolicy::Forbid,
            isolation: Isolation::None,
            enabled: true,
            entrypoint: None,
        }
    }

    #[test]
    fn qualified_targets_parse() {
        assert_eq!(
            parse_qualified("personal/digest"),
            Some(("personal", "digest"))
        );
        assert_eq!(parse_qualified("no-slash"), None);
        assert_eq!(parse_qualified("a/b/c"), None);
        assert_eq!(parse_qualified("/agent"), None);
        assert_eq!(parse_qualified("project/"), None);
    }

    #[test]
    fn run_requires_a_known_agent() {
        let store = temp_store("unknown-agent");
        let err = run(&store, "personal/digest", None, &[], None).unwrap_err();
        assert!(err.to_string().contains("agentctl apply"));
    }

    #[test]
    fn run_creates_a_pending_manual_run() {
        let mut store = temp_store("creates-pending");
        store
            .reconcile_agents(&[spec("personal", "digest")])
            .unwrap();
        run(&store, "personal/digest", None, &[], None).unwrap();
        let pending = store.pending_runs().unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].trigger, Trigger::Manual);
    }

    #[test]
    fn kill_rejects_non_running_runs() {
        let store = temp_store("kill-pending");
        let created = store
            .create_pending_run("p", "a", Trigger::Manual, None, None, None)
            .unwrap();
        let err = kill(&store, &created.id).unwrap_err();
        assert!(err.to_string().contains("pending"));
    }
}
