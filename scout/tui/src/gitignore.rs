//! Keeping session recordings out of git. Recordings and their rendered
//! copies hold raw command output, which can include secrets, so a store
//! inside a git repository must ignore them — whether it was made with
//! `beagle scout init` or picked through `$SCOUT_ROOT` / config `root`.

use std::path::Path;

/// Paths a store must keep out of git: session recordings and their
/// rendered copies.
pub const LINES: [&str; 2] = ["/.sessions/", "/*/sessions/"];

/// Makes sure `store`'s `.gitignore` lists [`LINES`]. Idempotent; never
/// removes or reorders a line.
///
/// # Errors
/// The `.gitignore` cannot be written.
pub fn ensure(store: &Path) -> std::io::Result<()> {
    let ignore = store.join(".gitignore");
    let existing = std::fs::read_to_string(&ignore).unwrap_or_default();
    let missing: Vec<&str> = LINES
        .into_iter()
        .filter(|line| !existing.lines().any(|l| l.trim() == *line))
        .collect();
    if missing.is_empty() {
        return Ok(());
    }
    let mut text = existing;
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str("# scout session recordings: raw command output, never commit\n");
    for line in missing {
        text.push_str(line);
        text.push('\n');
    }
    std::fs::write(&ignore, text)
}

/// Whether `dir` is inside a git work tree (a `.git` directory or file in
/// it or any ancestor).
#[must_use]
pub fn in_git_repo(dir: &Path) -> bool {
    dir.ancestors().any(|d| d.join(".git").exists())
}

#[cfg(test)]
// Tests fail loudly on setup errors; unwrap is the clearest way.
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn ensure_appends_once_and_keeps_existing_lines() {
        let dir = tempfile::tempdir().unwrap();
        let ignore = dir.path().join(".gitignore");
        std::fs::write(&ignore, "target/\n/.sessions/").unwrap();
        ensure(dir.path()).unwrap();
        ensure(dir.path()).unwrap();
        let text = std::fs::read_to_string(&ignore).unwrap();
        assert!(text.starts_with("target/\n/.sessions/\n"));
        assert_eq!(text.matches("/.sessions/").count(), 1);
        assert_eq!(text.matches("/*/sessions/").count(), 1);
    }

    #[test]
    fn git_repos_are_found_from_any_depth() {
        let dir = tempfile::tempdir().unwrap();
        let deep = dir.path().join("a/b");
        std::fs::create_dir_all(&deep).unwrap();
        assert!(!in_git_repo(&deep));
        std::fs::create_dir(dir.path().join(".git")).unwrap();
        assert!(in_git_repo(&deep));
    }
}
