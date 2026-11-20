//! Preparing a PR checkout for conflict resolution, and pushing the
//! result. The agent may not be able to reach the network (codex runs in
//! its workspace-write sandbox) and has no GitHub credentials either way, so
//! Tandem does the fetching and merging: by the time a run starts, the
//! base commits are present and the conflict markers are real files on
//! disk it can edit.

use beagle_utils::process::augmented_path;
use serde::Serialize;
use tandem_core::TandemError;

/// What a prepared merge left behind.
#[derive(Debug, Clone, Serialize)]
pub struct MergeState {
    /// Paths git could not merge on its own.
    pub conflicted: Vec<String>,
    /// True when the merge applied cleanly — nothing for an agent to do.
    pub clean: bool,
}

/// Bound on a git command that talks to the remote or runs hooks
/// (`fetch`, `push`, `commit`).
const GIT_REMOTE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);
/// Bound on a local git command.
const GIT_LOCAL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

pub(crate) async fn git(
    args: &[&str],
    dir: &std::path::Path,
) -> Result<(bool, String, String), TandemError> {
    let timeout = match args.first() {
        Some(&("fetch" | "push" | "commit")) => GIT_REMOTE_TIMEOUT,
        _ => GIT_LOCAL_TIMEOUT,
    };
    let mut cmd = tokio::process::Command::new("git");
    cmd.env("PATH", augmented_path())
        .args(args)
        .current_dir(dir)
        .kill_on_drop(true);
    // On expiry the future is dropped, which kills git (kill_on_drop).
    let output = tokio::time::timeout(timeout, cmd.output())
        .await
        .map_err(|_| {
            TandemError::Config(format!(
                "`git {}` timed out after {}s",
                args.first().copied().unwrap_or_default(),
                timeout.as_secs()
            ))
        })?
        .map_err(|e| TandemError::Config(format!("failed to run git: {e}")))?;
    Ok((
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).trim().to_owned(),
        String::from_utf8_lossy(&output.stderr).trim().to_owned(),
    ))
}

async fn must(args: &[&str], dir: &std::path::Path) -> Result<String, TandemError> {
    let (ok, out, err) = git(args, dir).await?;
    if ok {
        return Ok(out);
    }
    Err(TandemError::Config(format!(
        "`git {}` failed: {}",
        args.join(" "),
        if err.is_empty() { out } else { err }
    )))
}

/// Fetch the base branch and start a merge, leaving conflicts in the
/// tree. Any half-finished merge from a previous attempt is abandoned
/// first so a retry always starts from the branch as it stands.
pub(crate) async fn prepare_merge(
    dir: &std::path::Path,
    base_ref: &str,
) -> Result<MergeState, TandemError> {
    // An abort on a tree with no merge in progress is an error, not a
    // problem — ignore it.
    let _ = git(&["merge", "--abort"], dir).await?;
    must(&["fetch", "origin", base_ref], dir).await?;

    let (ok, _, _) = git(
        &[
            "-c",
            "merge.conflictStyle=zdiff3",
            "merge",
            "--no-commit",
            "--no-ff",
            "FETCH_HEAD",
        ],
        dir,
    )
    .await?;

    let unmerged = must(&["diff", "--name-only", "--diff-filter=U"], dir).await?;
    let conflicted: Vec<String> = unmerged.lines().map(str::to_owned).collect();
    Ok(MergeState {
        clean: ok && conflicted.is_empty(),
        conflicted,
    })
}

/// Is the merge resolved — every path staged and no markers left?
pub(crate) async fn unresolved(dir: &std::path::Path) -> Result<Vec<String>, TandemError> {
    let unmerged = must(&["diff", "--name-only", "--diff-filter=U"], dir).await?;
    Ok(unmerged.lines().map(str::to_owned).collect())
}
