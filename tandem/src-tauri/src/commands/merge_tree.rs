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

/// The paths git left unmerged: the conflicted files, whether or not the
/// agent has finished editing them (it is told not to touch git, so they
/// stay unstaged until the push).
pub(crate) async fn conflict_paths(dir: &std::path::Path) -> Result<Vec<String>, TandemError> {
    let unmerged = must(&["diff", "--name-only", "--diff-filter=U"], dir).await?;
    let mut paths: Vec<String> = unmerged.lines().map(str::to_owned).collect();
    paths.dedup();
    Ok(paths)
}

/// The conflicted paths that still hold a conflict marker — what is left
/// for a human. A conflicted path with no markers is resolved and waits
/// to be staged by the push.
pub(crate) async fn unresolved(dir: &std::path::Path) -> Result<Vec<String>, TandemError> {
    Ok(conflict_paths(dir)
        .await?
        .into_iter()
        .filter(|p| {
            // An unreadable or deleted file has no markers to resolve.
            std::fs::read(dir.join(p)).is_ok_and(|bytes| has_conflict_markers(&bytes))
        })
        .collect())
}

/// Whether `text` holds a git conflict marker: a line starting with
/// `<<<<<<< ` or `>>>>>>> ` (or exactly `<<<<<<<` / `>>>>>>>`). `=======`
/// alone is not enough — it is also a Markdown heading underline.
pub(crate) fn has_conflict_markers(text: &[u8]) -> bool {
    text.split(|&b| b == b'\n').any(|line| {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        ["<<<<<<<", ">>>>>>>"].iter().any(|m| {
            line.strip_prefix(m.as_bytes())
                .is_some_and(|rest| rest.is_empty() || rest.starts_with(b" "))
        })
    })
}

/// What the push will commit for the conflicted paths: their diff
/// against the PR head, shown to the user before they push.
pub(crate) async fn resolution_diff(dir: &std::path::Path) -> Result<String, TandemError> {
    let paths = conflict_paths(dir).await?;
    if paths.is_empty() {
        return Ok(String::new());
    }
    let mut args = vec!["diff", "HEAD", "--"];
    args.extend(paths.iter().map(String::as_str));
    must(&args, dir).await
}

#[cfg(test)]
mod tests {
    use super::has_conflict_markers;

    #[test]
    fn markers_are_the_angle_lines_not_the_equals() {
        assert!(has_conflict_markers(
            b"a\n<<<<<<< HEAD\nx\n=======\ny\n>>>>>>> base\n"
        ));
        assert!(has_conflict_markers(b">>>>>>>\r\n"));
        assert!(!has_conflict_markers(b"Title\n=======\n\nbody\n"));
        assert!(!has_conflict_markers(b"let s = \"<<<<<<<\";\n"));
        assert!(!has_conflict_markers(b"<<<<<<<<< not a marker\n"));
    }

    /// A real conflicted merge: before the edit the file is unresolved;
    /// after the agent removes the markers (without touching git) it is
    /// resolved but still a conflict path, so the push stages it — and
    /// only it, not a stray file next to it.
    #[test]
    #[allow(clippy::unwrap_used)]
    fn an_edited_conflict_is_resolved_and_staged_alone() {
        use super::{conflict_paths, git, resolution_diff, unresolved};
        let dir = std::env::temp_dir().join(format!("tandem-merge-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let run = |args: &'static [&'static str]| {
                let dir = dir.clone();
                async move { assert!(git(args, &dir).await.unwrap().0, "git {args:?}") }
            };
            run(&["init", "-q", "-b", "main"]).await;
            run(&["config", "user.email", "t@example.com"]).await;
            run(&["config", "user.name", "t"]).await;
            std::fs::write(dir.join("f.txt"), "a\n").unwrap();
            run(&["add", "f.txt"]).await;
            run(&["commit", "-q", "-m", "base"]).await;
            run(&["checkout", "-q", "-b", "side"]).await;
            std::fs::write(dir.join("f.txt"), "side\n").unwrap();
            run(&["commit", "-q", "-am", "side"]).await;
            run(&["checkout", "-q", "main"]).await;
            std::fs::write(dir.join("f.txt"), "main\n").unwrap();
            run(&["commit", "-q", "-am", "main"]).await;
            let (clean, _, _) = git(&["merge", "--no-commit", "side"], &dir).await.unwrap();
            assert!(!clean);

            assert_eq!(unresolved(&dir).await.unwrap(), vec!["f.txt"]);
            std::fs::write(dir.join("f.txt"), "main\nside\n").unwrap();
            std::fs::write(dir.join("stray.txt"), "scratch\n").unwrap();
            assert_eq!(unresolved(&dir).await.unwrap(), Vec::<String>::new());
            assert_eq!(conflict_paths(&dir).await.unwrap(), vec!["f.txt"]);
            let diff = resolution_diff(&dir).await.unwrap();
            assert!(diff.contains("+side"), "{diff}");
            assert!(!diff.contains("stray"), "{diff}");
        });
        let _ = std::fs::remove_dir_all(&dir);
    }
}
