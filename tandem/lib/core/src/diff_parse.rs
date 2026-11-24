//! Parser for unified diffs (the `application/vnd.github.v3.diff` format
//! GitHub returns for a pull request).

use crate::diff::{DiffLine, FileDiff, FileStatus, Hunk, LineKind};
use crate::error::TandemError;

/// Parse a full multi-file unified diff into structured [`FileDiff`]s.
pub fn parse_unified_diff(input: &str) -> Result<Vec<FileDiff>, TandemError> {
    let mut files = Vec::new();
    let mut current: Option<FileBuilder> = None;

    for line in input.lines() {
        if let Some(rest) = line.strip_prefix("diff --git ") {
            if let Some(f) = current.take() {
                files.push(f.finish());
            }
            current = Some(FileBuilder::from_git_header(rest));
            continue;
        }
        let Some(builder) = current.as_mut() else {
            continue;
        };
        builder.feed(line)?;
    }
    if let Some(f) = current.take() {
        files.push(f.finish());
    }
    Ok(files)
}

struct FileBuilder {
    file: FileDiff,
    hunk: Option<Hunk>,
    old_cursor: u64,
    new_cursor: u64,
}

impl FileBuilder {
    /// `rest` is the text after `diff --git `, i.e. `a/path b/path`.
    fn from_git_header(rest: &str) -> Self {
        let (old_path, new_path) = split_git_paths(rest);
        Self {
            file: FileDiff {
                old_path,
                new_path,
                status: FileStatus::Modified,
                hunks: Vec::new(),
                additions: 0,
                deletions: 0,
                is_binary: false,
            },
            hunk: None,
            old_cursor: 0,
            new_cursor: 0,
        }
    }

    fn feed(&mut self, line: &str) -> Result<(), TandemError> {
        if let Some(header) = line.strip_prefix("@@") {
            self.flush_hunk();
            self.hunk = Some(parse_hunk_header(header)?);
            if let Some(h) = &self.hunk {
                self.old_cursor = h.old_start;
                self.new_cursor = h.new_start;
            }
            return Ok(());
        }
        if self.hunk.is_none() {
            self.feed_file_header(line);
            return Ok(());
        }
        self.feed_hunk_line(line);
        Ok(())
    }

    /// Lines between `diff --git` and the first `@@` describe the file.
    fn feed_file_header(&mut self, line: &str) {
        if line.starts_with("new file mode") {
            self.file.status = FileStatus::Added;
        } else if line.starts_with("deleted file mode") {
            self.file.status = FileStatus::Removed;
        } else if line.starts_with("rename from") || line.starts_with("rename to") {
            self.file.status = FileStatus::Renamed;
        } else if line.starts_with("Binary files") || line.starts_with("GIT binary patch") {
            self.file.is_binary = true;
        } else if let Some(p) = line.strip_prefix("--- ") {
            if p != "/dev/null" {
                self.file.old_path = strip_prefix_marker(p);
            }
        } else if let Some(p) = line.strip_prefix("+++ ") {
            if p != "/dev/null" {
                self.file.new_path = strip_prefix_marker(p);
            }
        }
    }

    fn feed_hunk_line(&mut self, line: &str) {
        let Some(hunk) = self.hunk.as_mut() else {
            return;
        };
        let (kind, content) = match line.split_at_checked(1) {
            Some(("+", rest)) => (LineKind::Added, rest),
            Some(("-", rest)) => (LineKind::Removed, rest),
            Some((" ", rest)) => (LineKind::Context, rest),
            // "\ No newline at end of file" and blank context lines.
            _ if line.is_empty() => (LineKind::Context, ""),
            _ => return,
        };
        let (old_line, new_line) = match kind {
            LineKind::Added => {
                self.file.additions += 1;
                let n = self.new_cursor;
                self.new_cursor += 1;
                (None, Some(n))
            }
            LineKind::Removed => {
                self.file.deletions += 1;
                let o = self.old_cursor;
                self.old_cursor += 1;
                (Some(o), None)
            }
            LineKind::Context => {
                let (o, n) = (self.old_cursor, self.new_cursor);
                self.old_cursor += 1;
                self.new_cursor += 1;
                (Some(o), Some(n))
            }
        };
        hunk.lines.push(DiffLine {
            kind,
            old_line,
            new_line,
            content: content.to_owned(),
        });
    }

    fn flush_hunk(&mut self) {
        if let Some(h) = self.hunk.take() {
            self.file.hunks.push(h);
        }
    }

    fn finish(mut self) -> FileDiff {
        self.flush_hunk();
        self.file
    }
}

/// Split `a/old b/new` from a `diff --git` header, handling the common
/// (unquoted, no-space) case; falls back to the whole string.
fn split_git_paths(rest: &str) -> (String, String) {
    let parts: Vec<&str> = rest.split(' ').collect();
    if parts.len() == 2 {
        (strip_prefix_marker(parts[0]), strip_prefix_marker(parts[1]))
    } else {
        (rest.to_owned(), rest.to_owned())
    }
}

/// Drop git's `a/` / `b/` prefix from a diff path.
fn strip_prefix_marker(p: &str) -> String {
    p.strip_prefix("a/")
        .or_else(|| p.strip_prefix("b/"))
        .unwrap_or(p)
        .to_owned()
}

/// Parse the `@@ -1,4 +2,6 @@ section` header (input starts after `@@`).
fn parse_hunk_header(header: &str) -> Result<Hunk, TandemError> {
    let bad = || TandemError::DiffParse(format!("malformed hunk header: @@{header}"));
    let (ranges, section) = match header.split_once("@@") {
        Some((r, s)) => (r.trim(), s.trim()),
        None => (header.trim(), ""),
    };
    let mut parts = ranges.split(' ');
    let old = parts
        .next()
        .and_then(|s| s.strip_prefix('-'))
        .ok_or_else(bad)?;
    let new = parts
        .next()
        .and_then(|s| s.strip_prefix('+'))
        .ok_or_else(bad)?;
    let (old_start, old_count) = parse_range(old).ok_or_else(bad)?;
    let (new_start, new_count) = parse_range(new).ok_or_else(bad)?;
    Ok(Hunk {
        old_start,
        old_count,
        new_start,
        new_count,
        section: section.to_owned(),
        lines: Vec::new(),
    })
}

/// `12,4` -> (12, 4); bare `12` -> (12, 1).
fn parse_range(s: &str) -> Option<(u64, u64)> {
    match s.split_once(',') {
        Some((a, b)) => Some((a.parse().ok()?, b.parse().ok()?)),
        None => Some((s.parse().ok()?, 1)),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
diff --git a/src/main.rs b/src/main.rs
index 1234567..89abcde 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,4 +1,5 @@ fn main
 fn main() {
-    println!(\"hello\");
+    println!(\"hello, tandem\");
+    println!(\"yip yip\");
 }
diff --git a/README.md b/README.md
new file mode 100644
index 0000000..e69de29
--- /dev/null
+++ b/README.md
@@ -0,0 +1,1 @@
+# Tandem
";

    #[test]
    fn parses_two_files() {
        let files = parse_unified_diff(SAMPLE).unwrap();
        assert_eq!(files.len(), 2);

        let main = &files[0];
        assert_eq!(main.new_path, "src/main.rs");
        assert_eq!(main.status, FileStatus::Modified);
        assert_eq!(main.additions, 2);
        assert_eq!(main.deletions, 1);
        assert_eq!(main.hunks[0].section, "fn main");

        let readme = &files[1];
        assert_eq!(readme.status, FileStatus::Added);
        assert_eq!(readme.new_path, "README.md");
        assert_eq!(readme.additions, 1);
    }

    #[test]
    fn line_numbers_advance_correctly() {
        let files = parse_unified_diff(SAMPLE).unwrap();
        let lines = &files[0].hunks[0].lines;
        // context "fn main() {" occupies old 1 / new 1
        assert_eq!(lines[0].old_line, Some(1));
        assert_eq!(lines[0].new_line, Some(1));
        // removed println is old line 2
        assert_eq!(lines[1].kind, LineKind::Removed);
        assert_eq!(lines[1].old_line, Some(2));
        // added lines are new 2 and 3
        assert_eq!(lines[2].new_line, Some(2));
        assert_eq!(lines[3].new_line, Some(3));
        // closing brace context: old 3 / new 4
        assert_eq!(lines[4].old_line, Some(3));
        assert_eq!(lines[4].new_line, Some(4));
    }

    #[test]
    fn binary_files_flagged() {
        let diff = "diff --git a/img.png b/img.png\nBinary files a/img.png and b/img.png differ\n";
        let files = parse_unified_diff(diff).unwrap();
        assert!(files[0].is_binary);
        assert!(files[0].hunks.is_empty());
    }

    #[test]
    fn malformed_hunk_header_errors() {
        let diff = "diff --git a/x b/x\n@@ nonsense @@\n";
        assert!(parse_unified_diff(diff).is_err());
    }
}
