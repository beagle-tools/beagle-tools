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

/// `export` prefix exposing the run's identity and context to the process:
/// `AGENTD_RUN_ID`/`PROJECT`/`AGENT`/`DATA_DIR`; for triggered runs
/// `AGENTD_PARENT_RUN_ID`/`PARENT_DATA_DIR`; `AGENTD_PARAM_<K>` for each
/// `--param`; `AGENTD_DB` when `database: true`; plus the manifest's `env:`
/// map. `~/.cargo/bin` is prepended to PATH so scripts can call `agentctl`.
pub(crate) fn env_exports(
    spec: &AgentSpec,
    run: &Run,
    data_dir: &Path,
    root: &str,
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
    let mut out = String::new();
    if let Some(home) = std::env::var_os("HOME") {
        let cargo_bin = PathBuf::from(home).join(".cargo/bin");
        let _ = write!(
            out,
            "export PATH={}:\"$PATH\"; ",
            shell_quote(&cargo_bin.to_string_lossy())
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
