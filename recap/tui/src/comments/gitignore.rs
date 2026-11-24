//! Keeping comment data out of git. `recaps/<id>/comments/` holds other
//! people's PR comments (re-fetchable) and your read marks and agent
//! reviews (yours alone); none of it belongs in a repo, so a store inside a
//! git repository gets ignore lines the first time it is written.

use std::path::Path;

/// What the store's `.gitignore` must list (relative to the store root).
pub const LINES: [&str; 2] = ["/recaps/*/comments/", "/recaps/.comments-error.txt"];

/// When the store holding `recap_dir` (`<root>/recaps/<id>`) is inside a
/// git repo, makes sure its `.gitignore` lists [`LINES`]. Idempotent; never
/// removes a line. Best effort: a store that cannot be written to will
/// fail on the comment write itself, with a better error.
pub fn ensure(recap_dir: &Path) {
    let Some(root) = recap_dir.parent().and_then(Path::parent) else {
        return;
    };
    if !root.ancestors().any(|d| d.join(".git").exists()) {
        return;
    }
    let ignore = root.join(".gitignore");
    let existing = std::fs::read_to_string(&ignore).unwrap_or_default();
    let missing: Vec<&str> = LINES
        .into_iter()
        .filter(|line| !existing.lines().any(|l| l.trim() == *line))
        .collect();
    if missing.is_empty() {
        return;
    }
    let mut text = existing;
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str("# recap comment data: other people's comments and your notes, never commit\n");
    for line in missing {
        text.push_str(line);
        text.push('\n');
    }
    let _ = beagle_utils::fs::write_atomic(&ignore, text.as_bytes());
}
