use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::paths::expand_tilde;
use crate::types::{AgentSpec, ConcurrencyPolicy, Isolation, NotifyPolicy, ProjectSpec, Runner};
use crate::CoreError;

/// Raw YAML shape of an agent manifest, before resolution/validation.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAgentManifest {
    kind: String,
    name: String,
    runner: Runner,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    schedule: Option<String>,
    workdir: PathBuf,
    #[serde(default)]
    prompt: Option<PathBuf>,
    #[serde(default)]
    command: Option<String>,
    #[serde(default)]
    allowed_tools: Option<Vec<String>>,
    #[serde(default)]
    disallowed_tools: Option<Vec<String>>,
    #[serde(default)]
    append_system_prompt: Option<PathBuf>,
    #[serde(default)]
    add_dirs: Vec<PathBuf>,
    #[serde(default)]
    params: Vec<crate::params::ParamSpec>,
    #[serde(default)]
    requires_mcp: Vec<String>,
    #[serde(default)]
    mcp_config: Option<PathBuf>,
    #[serde(default)]
    env: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    env_file: Option<PathBuf>,
    #[serde(default)]
    database: Option<bool>,
    #[serde(default)]
    max_concurrent: Option<u32>,
    #[serde(default)]
    noop_retention_minutes: Option<u64>,
    #[serde(default)]
    timeout_minutes: Option<u64>,
    #[serde(default)]
    notify: Option<NotifyPolicy>,
    #[serde(default)]
    notify_icon: Option<PathBuf>,
    #[serde(default)]
    notify_icon_failure: Option<PathBuf>,
    #[serde(default)]
    concurrency: Option<ConcurrencyPolicy>,
    #[serde(default)]
    isolation: Option<Isolation>,
    #[serde(default)]
    enabled: Option<bool>,
    entrypoint: Option<bool>,
}

/// Raw YAML shape of an optional `project.yaml`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProjectManifest {
    kind: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    description: Option<String>,
}

/// A problem found while loading one manifest file. Loading keeps going so a
/// single bad file never takes the whole fleet down.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestIssue {
    /// The offending manifest file.
    pub path: PathBuf,
    /// What is wrong with it, for a human.
    pub message: String,
}

/// Result of scanning a manifests root directory.
#[derive(Debug, Default)]
pub struct LoadOutcome {
    /// One per project directory that loaded.
    pub projects: Vec<ProjectSpec>,
    /// Every agent that loaded cleanly, fully resolved.
    pub agents: Vec<AgentSpec>,
    /// Files that were skipped, and why.
    pub issues: Vec<ManifestIssue>,
}

/// Scan `root/<project>/*.yaml` and produce resolved specs.
///
/// Each immediate subdirectory of `root` is a project (the "namespace"). An
/// optional `project.yaml` carries metadata; every other `*.yaml`/`*.yml` file
/// is an agent manifest. Files that fail to parse or validate are reported as
/// issues, not hard errors.
pub fn load_root(root: &Path) -> Result<LoadOutcome, CoreError> {
    let mut out = LoadOutcome::default();
    let entries = std::fs::read_dir(root).map_err(|e| CoreError::io(root, e))?;
    let mut project_dirs: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_dir() && !is_hidden(p))
        .collect();
    project_dirs.sort();

    for dir in project_dirs {
        load_project_dir(&dir, &mut out);
    }
    Ok(out)
}

fn load_project_dir(dir: &Path, out: &mut LoadOutcome) {
    let Some(project_name) = dir.file_name().and_then(|n| n.to_str()).map(str::to_owned) else {
        return;
    };
    if let Err(message) = validate_slug(&project_name) {
        out.issues.push(ManifestIssue {
            path: dir.to_path_buf(),
            message,
        });
        return;
    }

    let mut project = ProjectSpec {
        name: project_name.clone(),
        description: None,
    };
    let mut files: Vec<PathBuf> = match std::fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| is_yaml(p) && !is_hidden(p))
            .collect(),
        Err(e) => {
            out.issues.push(ManifestIssue {
                path: dir.to_path_buf(),
                message: e.to_string(),
            });
            return;
        }
    };
    files.sort();

    for file in files {
        if file.file_stem().is_some_and(|s| s == "project") {
            match load_project_meta(&file, &project_name) {
                Ok(description) => project.description = description,
                Err(e) => out.issues.push(ManifestIssue {
                    path: file,
                    message: e.to_string(),
                }),
            }
            continue;
        }
        match load_agent_file(&file, &project_name) {
            Ok(spec) => out.agents.push(spec),
            Err(e) => out.issues.push(ManifestIssue {
                path: file,
                message: e.to_string(),
            }),
        }
    }
    out.projects.push(project);
}

fn load_project_meta(path: &Path, project: &str) -> Result<Option<String>, CoreError> {
    let text = std::fs::read_to_string(path).map_err(|e| CoreError::io(path, e))?;
    let raw: RawProjectManifest =
        serde_yaml::from_str(&text).map_err(|e| CoreError::manifest(path, e.to_string()))?;
    if raw.kind != "Project" {
        return Err(CoreError::manifest(
            path,
            format!("kind must be Project, got {}", raw.kind),
        ));
    }
    if raw.name.as_ref().is_some_and(|n| n != project) {
        return Err(CoreError::manifest(
            path,
            format!("project name must match its directory name {project:?}"),
        ));
    }
    Ok(raw.description)
}

fn load_agent_file(path: &Path, project: &str) -> Result<AgentSpec, CoreError> {
    let text = std::fs::read_to_string(path).map_err(|e| CoreError::io(path, e))?;
    let raw: RawAgentManifest =
        serde_yaml::from_str(&text).map_err(|e| CoreError::manifest(path, e.to_string()))?;

    if raw.kind != "Agent" {
        return Err(CoreError::manifest(
            path,
            format!("kind must be Agent, got {}", raw.kind),
        ));
    }
    validate_slug(&raw.name).map_err(|m| CoreError::manifest(path, m))?;
    if let Some(expr) = &raw.schedule {
        parse_schedule(expr)?;
    }

    let workdir = expand_tilde(&raw.workdir)?;
    if !workdir.is_absolute() {
        return Err(CoreError::manifest(
            path,
            "workdir must be an absolute path (or ~/...)",
        ));
    }

    let base = path.parent().unwrap_or(Path::new("."));
    let prompt = validate_brain(&raw, base, path)?;

    if (raw.allowed_tools.is_some()
        || raw.disallowed_tools.is_some()
        || raw.append_system_prompt.is_some()
        || raw.mcp_config.is_some())
        && raw.runner != Runner::Claude
    {
        return Err(CoreError::manifest(
            path,
            "allowed_tools / append_system_prompt / mcp_config are only supported for the claude runner",
        ));
    }
    let mcp_config = resolve_existing_file(base, raw.mcp_config.as_ref(), path, "mcp_config")?;
    let append_system_prompt = resolve_existing_file(
        base,
        raw.append_system_prompt.as_ref(),
        path,
        "append_system_prompt file",
    )?;
    let mut add_dirs = Vec::new();
    for dir in &raw.add_dirs {
        add_dirs.push(expand_tilde(dir)?);
    }
    if raw.max_concurrent == Some(0) {
        return Err(CoreError::manifest(
            path,
            "max_concurrent must be at least 1",
        ));
    }

    let timeout_minutes = raw.timeout_minutes.unwrap_or(30);
    if timeout_minutes == 0 {
        return Err(CoreError::manifest(
            path,
            "timeout_minutes must be at least 1",
        ));
    }

    Ok(AgentSpec {
        project: project.to_owned(),
        name: raw.name,
        source: path.to_path_buf(),
        runner: raw.runner,
        model: raw.model,
        schedule: raw.schedule,
        workdir,
        prompt,
        command: raw.command,
        allowed_tools: raw.allowed_tools,
        disallowed_tools: raw.disallowed_tools,
        append_system_prompt,
        add_dirs,
        params: raw.params,
        requires_mcp: raw.requires_mcp,
        mcp_config,
        env: raw.env,
        env_file: resolve_existing_file(base, raw.env_file.as_ref(), path, "env_file")?,
        database: raw.database.unwrap_or(false),
        max_concurrent: raw.max_concurrent,
        noop_retention_minutes: raw.noop_retention_minutes,
        timeout_minutes,
        notify: raw.notify,
        notify_icon: resolve_opt(base, raw.notify_icon.as_ref())?,
        notify_icon_failure: resolve_opt(base, raw.notify_icon_failure.as_ref())?,
        concurrency: raw.concurrency.unwrap_or_default(),
        isolation: raw.isolation.unwrap_or_default(),
        enabled: raw.enabled.unwrap_or(true),
        entrypoint: raw.entrypoint,
    })
}

/// Resolve a manifest-relative path and require that it exists as a file.
/// `label` names the field in the error, e.g. `env_file not found: ...`.
fn resolve_existing_file(
    base: &Path,
    raw: Option<&PathBuf>,
    path: &Path,
    label: &str,
) -> Result<Option<PathBuf>, CoreError> {
    let resolved = resolve_opt(base, raw)?;
    if let Some(file) = &resolved {
        if !file.is_file() {
            return Err(CoreError::manifest(
                path,
                format!("{label} not found: {}", file.display()),
            ));
        }
    }
    Ok(resolved)
}

/// Check the runner has the right "brain" input: LLM runners take `prompt`
/// (file or directory of .md), the script runner takes `command`. Returns
/// the resolved prompt path for LLM runners.
fn validate_brain(
    raw: &RawAgentManifest,
    base: &Path,
    path: &Path,
) -> Result<Option<PathBuf>, CoreError> {
    if raw.runner == Runner::Script {
        if raw.command.as_deref().is_none_or(str::is_empty) {
            return Err(CoreError::manifest(path, "script runner requires command"));
        }
        if raw.prompt.is_some() {
            return Err(CoreError::manifest(
                path,
                "script runner takes command, not prompt",
            ));
        }
        return Ok(None);
    }
    if raw.command.is_some() {
        return Err(CoreError::manifest(
            path,
            "command is only for the script runner; LLM runners take prompt",
        ));
    }
    let Some(raw_prompt) = &raw.prompt else {
        return Err(CoreError::manifest(path, "prompt is required"));
    };
    let prompt = resolve_relative(base, &expand_tilde(raw_prompt)?);
    if prompt.is_dir() {
        // A directory prompt: every `.md` inside is concatenated (sorted by
        // name) at spawn time. It must not be empty now, though.
        if !dir_has_md(&prompt) {
            return Err(CoreError::manifest(
                path,
                format!("prompt directory has no .md files: {}", prompt.display()),
            ));
        }
    } else if !prompt.is_file() {
        return Err(CoreError::manifest(
            path,
            format!("prompt file not found: {}", prompt.display()),
        ));
    }
    Ok(Some(prompt))
}

pub use crate::manifest_util::parse_schedule;
use crate::manifest_util::{
    dir_has_md, is_hidden, is_yaml, resolve_opt, resolve_relative, validate_slug,
};

#[cfg(test)]
// Tests should fail loudly on a broken fixture; unwrapping is the clearest way.
#[allow(clippy::unwrap_used)]
#[path = "tests/manifest.rs"]
mod tests;
