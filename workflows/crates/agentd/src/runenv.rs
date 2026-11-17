//! Per-run environment and prompt context: the `export` prefix every run
//! script gets, and the literal "Run context" section appended to LLM
//! prompts (scoped tool rules deny `$VAR` expansions, so LLM params must be
//! literal text, not environment variables).

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use agent_core::paths;
use agent_core::types::{AgentSpec, Run};
use anyhow::{Context, Result};

use beagle_utils::shell::quote as shell_quote;

/// The literal run context appended to every LLM prompt: identity, data
/// dirs, and each `--param` spelled out. Values are literals on purpose —
/// see the scoped-allowlist note above.
pub(crate) fn run_context(run: &Run, data_dir: &Path, root: &str) -> String {
    let mut out =
        String::from("\n\n## Run context (from agentd — literal values, use them as-is)\n\n");
    let _ = writeln!(out, "- run id: {}", run.id);
    let _ = writeln!(out, "- data dir (scratch/output): {}", data_dir.display());
    // The chain root, resolved by the daemon from the store. A stage
    // that writes a per-chain artifact (the handover write-up) keys it
    // on this — it must never be derived from another id by string
    // surgery, which produces a plausible id for a run that does not
    // exist.
    let _ = writeln!(out, "- chain root run id: {root}");
    if let Some(parent) = &run.parent_run_id {
        let _ = writeln!(out, "- parent run: {parent}");
        if let Some(root) = data_dir.parent() {
            let _ = writeln!(out, "- parent data dir: {}", root.join(parent).display());
        }
    }
    // The prompt gets the params under the names the MANIFEST declares, not
    // the uppercased env-safe form — a prompt that says "the `url` param"
    // should not find `URL:` in its context.
    for (key, value) in declared_pairs(run.params.as_deref()) {
        let _ = writeln!(out, "- {key}: {value}");
    }
    out.push_str(
        "\nDo NOT read these from environment variables — your tool allowlist \
         denies commands containing `$VAR` expansions. Substitute the literal \
         values above directly into any command you run.\n",
    );
    out
}

/// The job a run belongs to, for its prompt and environment: the chain
/// root's `project/agent` and that job's `job-purpose.md`.
#[derive(Debug, Clone)]
pub(crate) struct JobPurpose {
    pub job: String,
    pub path: PathBuf,
}

/// Lines of a purpose file put in the prompt, at most.
const PURPOSE_BRIEF_LINES: usize = 150;

/// The top of every model run's prompt: what the whole job is for and
/// where this step fits, before the step's own instructions. The purpose
/// file's opening sections — up to "Where things live", which with the
/// failure modes and repair rules is for whoever fixes the job — plus the
/// path to the whole file. `None` when the file cannot be read.
pub(crate) fn purpose_brief(run: &Run, purpose: &JobPurpose) -> Option<String> {
    let text = std::fs::read_to_string(&purpose.path).ok()?;
    let mut brief = Vec::new();
    for line in text.lines().take(PURPOSE_BRIEF_LINES) {
        if line.starts_with("## Where things live") || line.starts_with("## Known failure modes") {
            break;
        }
        brief.push(line);
    }
    let mut out = String::from(
        "# The job you are part of — read this first

",
    );
    let _ = writeln!(
        out,
        "You are the `{}/{}` step of the job `{}`. Below is what the whole job \
         is for and how its steps fit together, so you know where your part \
         sits before you read your own instructions. Do your step, not the \
         job's other steps. The full purpose file (where things live, known \
         failure modes, the rules the job keeps) is `{}`; read it when you \
         need more.\n\n---\n",
        run.project,
        run.agent,
        purpose.job,
        purpose.path.display()
    );
    out.push_str(&brief.join("\n"));
    out.push_str("\n\n---\n");
    Some(out)
}

/// `export` prefix exposing the run's identity and context to the process:
/// `AGENTD_RUN_ID`/`PROJECT`/`AGENT`/`DATA_DIR`; for triggered runs
/// `AGENTD_PARENT_RUN_ID`/`PARENT_DATA_DIR`; `AGENTD_PARAM_<K>` for each
/// `--param`; `AGENTD_DB` when `database: true`; plus the manifest's `env:`
/// map. `~/.cargo/bin` and `~/.local/bin` are prepended to PATH: the daemon
/// runs under launchd, whose PATH has neither, so without them a run cannot
/// call `agentctl` or a user-installed CLI like `beagle`.
pub(crate) fn env_exports(
    spec: &AgentSpec,
    run: &Run,
    data_dir: &Path,
    root: &str,
    purpose: Option<&JobPurpose>,
) -> Result<String> {
    let mut vars = vec![
        format!("AGENTD_RUN_ID={}", shell_quote(&run.id)),
        format!("AGENTD_PROJECT={}", shell_quote(&run.project)),
        format!("AGENTD_AGENT={}", shell_quote(&run.agent)),
        format!(
            "AGENTD_DATA_DIR={}",
            shell_quote(&data_dir.to_string_lossy())
        ),
    ];
    vars.push(format!("AGENTD_ROOT_RUN_ID={}", shell_quote(root)));
    // The job's job-purpose.md, for a script that wants to point at it.
    if let Some(purpose) = purpose {
        vars.push(format!(
            "AGENTD_JOB_PURPOSE={}",
            shell_quote(&purpose.path.to_string_lossy())
        ));
    }
    // The standard logger, for script stages: `. "$AGENTD_LOG_SH"`, then
    // log_info / log_step / log_ok / log_warn / log_error.
    if let Ok(log_sh) = crate::loglib::path() {
        vars.push(format!(
            "AGENTD_LOG_SH={}",
            shell_quote(&log_sh.to_string_lossy())
        ));
    }
    if let Some(parent) = &run.parent_run_id {
        vars.push(format!("AGENTD_PARENT_RUN_ID={}", shell_quote(parent)));
        if let Some(dirs) = data_dir.parent() {
            let parent_dir = dirs.join(parent);
            vars.push(format!(
                "AGENTD_PARENT_DATA_DIR={}",
                shell_quote(&parent_dir.to_string_lossy())
            ));
        }
    }
    if spec.database {
        let db = paths::state_dir()
            .context("resolve state dir")?
            .join("db")
            .join(format!("{}-{}.sqlite", run.project, run.agent));
        if let Some(dir) = db.parent() {
            std::fs::create_dir_all(dir).context("create agent db dir")?;
        }
        vars.push(format!("AGENTD_DB={}", shell_quote(&db.to_string_lossy())));
    }
    for (key, value) in param_pairs(run.params.as_deref()) {
        vars.push(format!("AGENTD_PARAM_{key}={}", shell_quote(&value)));
    }
    for (key, value) in &spec.env {
        vars.push(format!("{key}={}", shell_quote(value)));
    }
    // The git guard (see gitguard.rs): the orgs its pre-push hook protects
    // (always set, so a run never inherits a stale list), and hooksPath for
    // every git call, even one that bypasses the shim on PATH.
    vars.push(format!(
        "AGENTD_PROTECTED_ORGS={}",
        shell_quote(&agent_core::guard::protected_orgs().join(" "))
    ));
    if let Ok(hooks) = crate::gitguard::hooks_dir() {
        vars.push("GIT_CONFIG_COUNT=1".to_owned());
        vars.push("GIT_CONFIG_KEY_0=core.hooksPath".to_owned());
        vars.push(format!(
            "GIT_CONFIG_VALUE_0={}",
            shell_quote(&hooks.to_string_lossy())
        ));
    }
    let mut out = String::new();
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        let cargo_bin = home.join(".cargo/bin");
        let local_bin = home.join(".local/bin");
        let _ = write!(
            out,
            "export PATH={}:{}:\"$PATH\"; ",
            shell_quote(&cargo_bin.to_string_lossy()),
            shell_quote(&local_bin.to_string_lossy())
        );
    }
    // The git shim goes in front of everything (see gitguard.rs).
    if let Ok(bin) = crate::gitguard::bin_dir() {
        let _ = write!(
            out,
            "export PATH={}:\"$PATH\"; ",
            shell_quote(&bin.to_string_lossy())
        );
    }
    // Secrets file first, so the manifest's explicit `env:` wins on clashes.
    if let Some(file) = &spec.env_file {
        let _ = write!(
            out,
            "set -a; . {}; set +a; ",
            shell_quote(&file.to_string_lossy())
        );
    }
    let _ = write!(out, "export {}; ", vars.join(" "));
    Ok(out)
}

/// Decode a run's `params` JSON object into (name, value) pairs, keeping the
/// names exactly as the manifest declared them.
pub(crate) fn declared_pairs(params: Option<&str>) -> Vec<(String, String)> {
    let Some(json) = params.and_then(|p| serde_json::from_str::<serde_json::Value>(p).ok()) else {
        return Vec::new();
    };
    let Some(map) = json.as_object() else {
        return Vec::new();
    };
    map.iter()
        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_owned())))
        .collect()
}

/// The same pairs with env-safe keys: uppercased, non-alphanumerics mapped
/// to `_`, for `AGENTD_PARAM_<K>`.
pub(crate) fn param_pairs(params: Option<&str>) -> Vec<(String, String)> {
    declared_pairs(params)
        .into_iter()
        .map(|(k, v)| {
            let key: String = k
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() {
                        c.to_ascii_uppercase()
                    } else {
                        '_'
                    }
                })
                .collect();
            (key, v)
        })
        .collect()
}

#[cfg(test)]
// Tests should fail loudly on a broken fixture; unwrapping is the clearest way.
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn the_brief_is_the_big_picture_not_the_repair_notes() {
        let dir = std::env::temp_dir().join(format!("agentd-purpose-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("job-purpose.md");
        std::fs::write(
            &path,
            "# p/job\n\nTurns tickets into PRs.\n\n## Steps\n\n| gate | ... |\n\n\
             ## Where things live\n\n- secrets of the trade\n\n## Known failure modes\n\n- oops\n",
        )
        .unwrap();
        let run: Run = serde_json::from_str(
            r#"{"id":"step-1","project":"p","agent":"step","state":"pending","trigger":"manual",
               "parent_run_id":null,"params":null,"tmux_session":null,"log_path":null,"workdir":null,
               "model":null,"created_at":"2026-10-27T00:00:00Z","started_at":null,"finished_at":null,
               "exit_code":null,"cost_usd":null,"num_turns":null,"usage_json":null}"#,
        )
        .unwrap();
        let brief = purpose_brief(
            &run,
            &JobPurpose {
                job: "p/job".to_owned(),
                path: path.clone(),
            },
        )
        .unwrap();
        assert!(brief.contains(
            "You are the `p/step` step of the job `p/job`. Below is what the whole job is for"
        ));
        assert!(brief.contains("Turns tickets into PRs."));
        assert!(brief.contains("| gate | ... |"));
        assert!(!brief.contains("secrets of the trade"));
        assert!(!brief.contains("oops"));
        assert!(brief.contains(&path.display().to_string()));
        let _ = std::fs::remove_dir_all(dir);
    }
}
