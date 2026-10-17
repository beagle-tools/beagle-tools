//! The side-effecting edges of a job: publishing a finished worktree (`git
//! push` + `gh pr create`) and the passive auth precheck. Both shell out, so
//! they live apart from the state machine, which tests drive with fakes.

use std::path::Path;
use std::process::Command;

use crate::worktree::Worktree;

/// The request handed to a [`Publisher`]: everything needed to push a branch and
/// open a PR for one RCA.
#[derive(Debug, Clone, Copy)]
pub struct PrRequest<'a> {
    /// The RCA id being remediated.
    pub rca_id: &'a str,
    /// The RCA title, for the PR title/body.
    pub title: Option<&'a str>,
    /// The worktree whose branch should be pushed.
    pub worktree: &'a Worktree,
}

/// A failure pushing a branch or opening a PR.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct PublishError(pub String);

/// Publishes a completed worktree: pushes its branch and opens a PR, returning
/// the PR URL. Abstracted so the orchestration can be tested without GitHub.
pub trait Publisher: Send + Sync {
    /// Pushes the branch and opens a PR, returning its URL.
    ///
    /// # Errors
    /// Returns [`PublishError`] if the push or PR creation fails.
    fn publish(&self, request: &PrRequest) -> Result<String, PublishError>;
}

/// The real publisher: `git push` from the worktree, then `gh pr create`.
#[derive(Debug)]
pub struct GhPublisher;

impl Publisher for GhPublisher {
    fn publish(&self, request: &PrRequest) -> Result<String, PublishError> {
        run_git(
            &request.worktree.path,
            &["push", "--set-upstream", "origin", &request.worktree.branch],
        )?;
        let title = match request.title {
            Some(title) => format!("agent: remediate {} — {title}", request.rca_id),
            None => format!("agent: remediate {}", request.rca_id),
        };
        let body = format!(
            "Automated remediation for RCA `{}`, opened by beagle-agentd.",
            request.rca_id
        );
        let output = Command::new("gh")
            .current_dir(&request.worktree.path)
            .args(["pr", "create", "--head"])
            .arg(&request.worktree.branch)
            .arg("--title")
            .arg(&title)
            .arg("--body")
            .arg(&body)
            .output()
            .map_err(|err| PublishError(format!("running gh: {err}")))?;
        if !output.status.success() {
            return Err(PublishError(format!(
                "gh pr create failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}

/// The default passive auth precheck: `gh auth status` and `claude --version`
/// must both succeed. Neither triggers an interactive login.
#[must_use]
pub fn default_precheck() -> Option<String> {
    if !tool_ok("gh", &["auth", "status"]) {
        return Some("gh is not authenticated".to_string());
    }
    if !tool_ok("claude", &["--version"]) {
        return Some("claude is not available".to_string());
    }
    None
}

/// Whether running `bin args...` exits zero.
fn tool_ok(bin: &str, args: &[&str]) -> bool {
    Command::new(bin)
        .args(args)
        .output()
        .is_ok_and(|out| out.status.success())
}

/// Runs `git -C <dir> <args...>`, mapping a non-zero exit to a [`PublishError`].
fn run_git(dir: &Path, args: &[&str]) -> Result<(), PublishError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .map_err(|err| PublishError(format!("running git: {err}")))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(PublishError(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        )))
    }
}
