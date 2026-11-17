//! Every job explains itself: `<project>/jobs/<job>/job-purpose.md`.
//!
//! A job — an agent somebody (or cron) starts, as opposed to a step another
//! agent triggers — must have a purpose file, or it does not load: it is
//! reported as a manifest issue and cannot run. The file says what the job
//! is for, its steps, what each reads and writes, and how it fails. People
//! read it in the repo; the pipeline resolver reads it before repairing a
//! job that broke, so it never has to rediscover a pipeline from scratch.
//!
//! Steps need none: they are described in their job's file. A step may
//! still have its own, and it is recorded the same way.

use std::path::{Path, PathBuf};

use crate::manifest::{LoadOutcome, ManifestIssue};
use crate::types::AgentSpec;

/// The file every job must have.
pub const PURPOSE_FILE: &str = "job-purpose.md";

/// Where `job`'s purpose file lives in its project directory.
pub fn purpose_path(project_dir: &Path, job: &str) -> PathBuf {
    project_dir.join("jobs").join(job).join(PURPOSE_FILE)
}

/// Is `spec` a job, judged from the manifests alone? Declared
/// `entrypoint: true`, or scheduled, or — unless declared
/// `entrypoint: false` — not named in any sibling's `triggers`.
///
/// Static on purpose: `pipelines::is_entry` also learns from run history,
/// but whether a job may load cannot depend on whether it has run.
pub fn is_job(spec: &AgentSpec, project: &[&AgentSpec]) -> bool {
    match spec.entrypoint {
        Some(declared) => declared || spec.schedule.is_some(),
        None => {
            spec.schedule.is_some()
                || !project
                    .iter()
                    .any(|other| other.name != spec.name && other.triggers.contains(&spec.name))
        }
    }
}

/// Record each of `project`'s agents' purpose file, and drop every job
/// that has none — with an issue saying exactly where it goes.
pub(crate) fn enforce(project_dir: &Path, project: &str, out: &mut LoadOutcome) {
    let jobs: Vec<String> = {
        let mine: Vec<&AgentSpec> = out.agents.iter().filter(|a| a.project == project).collect();
        mine.iter()
            .filter(|a| is_job(a, &mine))
            .map(|a| a.name.clone())
            .collect()
    };
    let mut missing = Vec::new();
    for spec in out.agents.iter_mut().filter(|a| a.project == project) {
        let path = purpose_path(project_dir, &spec.name);
        if path.is_file() {
            spec.purpose = Some(path);
        } else if jobs.contains(&spec.name) {
            missing.push((spec.name.clone(), spec.source.clone(), path));
        }
    }
    for (name, source, path) in missing {
        out.agents
            .retain(|a| !(a.project == project && a.name == name));
        out.issues.push(ManifestIssue {
            path: source,
            message: format!(
                "job {project}/{name} has no {} — every job needs one (what it is for, its steps, \
                 what each reads and writes, how it fails); it will not load until it exists",
                path.display()
            ),
        });
    }
}

#[cfg(test)]
// Tests should fail loudly on a broken fixture; unwrapping is the clearest way.
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn spec(name: &str, triggers: &[&str], entrypoint: Option<bool>, schedule: bool) -> AgentSpec {
        let mut spec: AgentSpec = serde_json::from_str(&format!(
            r#"{{"project":"p","name":"{name}","source":"/m/{name}.yaml","runner":"script","workdir":"/","timeout_minutes":1}}"#
        ))
        .unwrap();
        spec.triggers = triggers.iter().map(|t| (*t).to_owned()).collect();
        spec.entrypoint = entrypoint;
        spec.schedule = schedule.then(|| "0 * * * *".to_owned());
        spec
    }

    #[test]
    fn a_job_is_what_nothing_else_triggers_unless_declared() {
        let gate = spec("gate", &["step"], None, false);
        let step = spec("step", &[], None, false);
        let declared_step = spec("lone", &[], Some(false), false);
        let cron_step = spec("tick", &[], Some(false), true);
        let all = [&gate, &step, &declared_step, &cron_step];
        assert!(is_job(&gate, &all));
        assert!(!is_job(&step, &all));
        assert!(!is_job(&declared_step, &all));
        // A schedule makes it a job whatever it says: cron starts it.
        assert!(is_job(&cron_step, &all));
    }

    #[test]
    fn a_job_without_its_purpose_file_does_not_load() {
        let dir = std::env::temp_dir().join(format!("agent-core-purpose-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("jobs/documented")).unwrap();
        std::fs::write(purpose_path(&dir, "documented"), "# documented\n").unwrap();

        let mut out = LoadOutcome {
            agents: vec![
                spec("documented", &["step"], None, false),
                spec("step", &[], None, false),
                spec("undocumented", &[], None, false),
            ],
            ..LoadOutcome::default()
        };
        enforce(&dir, "p", &mut out);

        let names: Vec<&str> = out.agents.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, vec!["documented", "step"]);
        assert_eq!(
            out.agents[0].purpose,
            Some(purpose_path(&dir, "documented"))
        );
        assert_eq!(out.agents[1].purpose, None);
        assert_eq!(out.issues.len(), 1);
        assert!(out.issues[0].message.contains("p/undocumented has no"));

        let _ = std::fs::remove_dir_all(dir);
    }
}
