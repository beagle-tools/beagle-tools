//! The repo's own rules, handed to the reviewing agent. A review that
//! doesn't know the house style either misses real violations or
//! invents ones the project settled long ago — and these files are
//! exactly where projects write that down.

use std::fmt::Write as _;
use std::path::Path;

/// Where projects state their conventions, best-first. Everything here
/// is a file a human wrote for whoever works on the repo next.
const CONVENTION_FILES: &[&str] = &[
    "AGENTS.md",
    "CLAUDE.md",
    "CONTRIBUTING.md",
    "CONVENTIONS.md",
    "STYLE.md",
    "docs/STYLE.md",
    ".cursorrules",
    ".github/CONTRIBUTING.md",
];

/// Per-file and total budgets. A convention doc that runs past this is
/// usually appendix material, and the diff still needs room in the
/// prompt — the agent can open the file from its checkout if it wants
/// the rest.
const PER_FILE_CHARS: usize = 6_000;
const TOTAL_CHARS: usize = 18_000;

/// One convention file found in the checkout, clipped to the prompt budget.
#[derive(Debug, Clone)]
pub struct ConventionDoc {
    /// Repo-relative path, e.g. `AGENTS.md`.
    pub path: String,
    /// File contents, cut at the per-file / total budget.
    pub text: String,
    /// True when the file was longer than the budget allowed.
    pub truncated: bool,
}

/// Read whichever convention files the checkout actually has.
pub fn collect(root: &Path) -> Vec<ConventionDoc> {
    let mut docs = Vec::new();
    let mut budget = TOTAL_CHARS;
    for name in CONVENTION_FILES {
        if budget == 0 {
            break;
        }
        let Ok(raw) = std::fs::read_to_string(root.join(name)) else {
            continue;
        };
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        let cap = PER_FILE_CHARS.min(budget);
        // Char boundaries, not bytes — these files are full of em
        // dashes and arrows.
        let text: String = trimmed.chars().take(cap).collect();
        let truncated = text.chars().count() < trimmed.chars().count();
        budget -= text.chars().count().min(budget);
        docs.push(ConventionDoc {
            path: (*name).to_owned(),
            text,
            truncated,
        });
    }
    docs
}

/// The prompt section, or nothing at all when the repo states no
/// conventions — an empty heading would just invite invention.
pub fn block(docs: &[ConventionDoc]) -> String {
    if docs.is_empty() {
        return String::new();
    }
    let mut out = String::from(
        "## This repo's conventions\n\
         These are the project's own rules, taken from its checkout. \
         Hold the diff to them: when a comment is about one of these, \
         say which rule and set the comment's `tag` to \
         \"coding-standards\". Do not invent standards that aren't \
         stated here or visible in the surrounding code, and don't \
         repeat a rule the diff already follows.\n\n",
    );
    for doc in docs {
        let _ = write!(out, "### {}\n{}\n", doc.path, doc.text);
        if doc.truncated {
            out.push_str("\n[truncated — open the file in the checkout for the rest]\n");
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn scratch() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("tandem-conv-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn reads_only_the_files_that_exist() {
        let dir = scratch();
        std::fs::write(dir.join("AGENTS.md"), "no unwrap outside tests").unwrap();
        let docs = collect(&dir);
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].path, "AGENTS.md");
        assert!(!docs[0].truncated);
        assert!(block(&docs).contains("no unwrap outside tests"));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn truncates_a_long_file_and_says_so() {
        let dir = scratch();
        std::fs::write(
            dir.join("CONTRIBUTING.md"),
            "x".repeat(PER_FILE_CHARS + 500),
        )
        .unwrap();
        let docs = collect(&dir);
        assert!(docs[0].truncated);
        assert_eq!(docs[0].text.chars().count(), PER_FILE_CHARS);
        assert!(block(&docs).contains("truncated"));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn no_conventions_means_no_prompt_section() {
        let dir = scratch();
        assert!(collect(&dir).is_empty());
        assert_eq!(block(&[]), "");
        std::fs::remove_dir_all(dir).ok();
    }
}
