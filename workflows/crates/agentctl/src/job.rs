//! `agentctl -j <job> -p k=v,...` — run a declared job by name.
//!
//! A "job" is just an agent addressed by name rather than by
//! `<project>/<agent>`, with its `params:` block enforced before the run is
//! created. Nothing is spawned until the parameters check out, so a missing
//! or misspelled one fails at the prompt with an explanation instead of
//! halfway through a headless run.

use agent_core::params::{self, ParamSpec};
use agent_core::state::Store;
use agent_core::types::{AgentSpec, Trigger};
use anyhow::{anyhow, bail, Result};

/// Find the agent a `-j` name refers to: `<project>/<agent>`, or a bare
/// `<agent>` when exactly one project declares that name.
pub fn resolve(store: &Store, job: &str) -> Result<AgentSpec> {
    let agents = store.list_agents()?;
    if agents.is_empty() {
        bail!("no agents in the store; run `agentctl apply` to sync manifests");
    }
    if let Some((project, name)) = job.split_once('/') {
        return agents
            .into_iter()
            .find(|a| a.project == project && a.name == name)
            .ok_or_else(|| anyhow!("no job {job:?}; list them with `agentctl get jobs`"));
    }
    let mut matches: Vec<AgentSpec> = agents.into_iter().filter(|a| a.name == job).collect();
    match matches.len() {
        0 => Err(anyhow!(
            "no job {job:?}; list them with `agentctl get jobs`"
        )),
        1 => Ok(matches.remove(0)),
        _ => {
            let names: Vec<String> = matches.iter().map(AgentSpec::qualified_name).collect();
            Err(anyhow!(
                "job {job:?} is ambiguous — qualify it as one of: {}",
                names.join(", ")
            ))
        }
    }
}

/// Validate `-p` entries against the job's declared params and create the
/// pending run. `model` overrides the manifest's model for this run only.
pub fn run(store: &Store, job: &str, entries: &[String], model: Option<&str>) -> Result<()> {
    let spec = resolve(store, job)?;
    let supplied = params::parse_pairs(entries)?;
    // Report against the name the caller typed, so the suggested fix is
    // copy-pasteable as-is.
    let resolved = params::resolve(job, &spec.params, supplied)?;
    // Params first (free), then the MCP preflight (shells out per server).
    crate::preflight::check_mcp(job, &spec.requires_mcp)?;

    let params_json = if resolved.is_empty() {
        None
    } else {
        Some(serde_json::to_string(&resolved)?)
    };
    let created = store.create_pending_run(
        &spec.project,
        &spec.name,
        Trigger::Manual,
        None,
        params_json.as_deref(),
        model,
    )?;
    println!("{}", created.id);
    println!("watch with: agentctl logs {} -f", created.id);
    Ok(())
}

/// One row per job for `agentctl get jobs`: the qualified name plus its
/// declared parameters, so a caller can discover what a job needs without
/// having to trigger it and read the failure.
pub fn describe_params(specs: &[ParamSpec]) -> String {
    if specs.is_empty() {
        return "-".to_owned();
    }
    specs
        .iter()
        .map(|s| {
            if s.required {
                format!("{}*", s.name)
            } else if let Some(default) = &s.default {
                format!("{}={default}", s.name)
            } else {
                s.name.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
// Tests should fail loudly on a broken fixture; unwrapping is the clearest way.
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use agent_core::types::{ConcurrencyPolicy, Isolation, Runner};

    fn temp_store(tag: &str) -> Store {
        let path =
            std::env::temp_dir().join(format!("agentctl-job-{tag}-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        Store::open(&path).unwrap()
    }

    fn spec(project: &str, name: &str, params: Vec<ParamSpec>) -> AgentSpec {
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
            params,
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

    fn url_param() -> ParamSpec {
        ParamSpec {
            name: "url".to_owned(),
            required: true,
            default: None,
            description: Some("the ticket url".to_owned()),
        }
    }

    #[test]
    fn bare_names_resolve_and_ambiguity_is_reported() {
        let mut store = temp_store("resolve");
        store
            .reconcile_agents(&[
                spec("personal", "hello", vec![]),
                spec("work", "hello", vec![]),
                spec("personal", "unique", vec![]),
            ])
            .unwrap();
        assert_eq!(resolve(&store, "unique").unwrap().project, "personal");
        assert_eq!(resolve(&store, "work/hello").unwrap().project, "work");

        let err = resolve(&store, "hello").unwrap_err().to_string();
        assert!(err.contains("ambiguous"), "{err}");
        assert!(err.contains("personal/hello") && err.contains("work/hello"));
        assert!(resolve(&store, "nope")
            .unwrap_err()
            .to_string()
            .contains("get jobs"));
    }

    #[test]
    fn a_missing_required_param_creates_no_run() {
        let mut store = temp_store("missing-param");
        store
            .reconcile_agents(&[spec("personal", "linear", vec![url_param()])])
            .unwrap();
        let err = run(&store, "linear", &[], None).unwrap_err().to_string();
        assert!(err.contains("missing required parameter"), "{err}");
        assert!(err.contains("the ticket url"), "{err}");
        // Nothing was queued — the failure is at submit time.
        assert_eq!(
            store.pending_runs().unwrap(),
            [] as [agent_core::types::Run; 0]
        );
    }

    #[test]
    fn supplying_the_param_queues_the_run() {
        let mut store = temp_store("good-param");
        store
            .reconcile_agents(&[spec("personal", "linear", vec![url_param()])])
            .unwrap();
        run(
            &store,
            "linear",
            &["url=https://x/y".to_owned()],
            Some("opus"),
        )
        .unwrap();
        let pending = store.pending_runs().unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].model.as_deref(), Some("opus"));
        assert!(pending[0]
            .params
            .as_deref()
            .unwrap()
            .contains("https://x/y"));
    }

    #[test]
    fn param_summaries_mark_required_and_defaults() {
        assert_eq!(describe_params(&[]), "-");
        assert_eq!(
            describe_params(&[
                url_param(),
                ParamSpec {
                    name: "depth".to_owned(),
                    required: false,
                    default: Some("2".to_owned()),
                    description: None,
                },
            ]),
            "url*,depth=2"
        );
    }
}
