//! Which repos a model run changed, read from the commands it ran.
//!
//! Scripts log every repo they touch themselves (`lib/repo-log.sh` in the
//! manifests); a model run's commands are in its event feed instead. When it
//! finishes, agentd reads every Bash tool call, keeps the ones that write —
//! a commit, push, worktree change, PR create/merge/review/comment, an API
//! write — works out which repo each one hit, and records one line, so
//! `agentctl logs` shows exactly which repos a run touched:
//!
//! ```text
//! repos touched: acme/widgets @ abc-123-fix (commit ×2, push) · jdoe/notes @ main (push)
//! ```
//!
//! Each repo is split by the branch the write hit: a push's refspec, a PR's
//! `--head`, else the checkout's current branch (the caller resolves that).
//! A push to `main`/`master` on a protected org (see [`crate::guard`]; none
//! unless configured) is flagged `⚠ PUSHED TO MAIN` — the git guard
//! (agentd's `gitguard`) should have refused it, so seeing one means a hole.
//!
//! Pure: [`writes_in`] parses; resolving a local checkout to `owner/name`
//! (it needs `git`) is the caller's job.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::types::Event;

/// Where a write landed.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Target {
    /// A local checkout (resolve it with its origin remote).
    Local(PathBuf),
    /// A GitHub repo named on the command line: `owner/name`.
    GitHub(String),
}

/// One write: where, on which branch when the command says, and what
/// (`push`, `commit`, `pr create`, …).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Write {
    /// The checkout or GitHub repo written to.
    pub target: Target,
    /// From the command itself: a push refspec's destination, a PR's
    /// `--head`. `None` means "the checkout's current branch" for a local
    /// write, or unknown.
    pub branch: Option<String>,
    /// What the write was: `push`, `commit`, `pr create`, `api write`, …
    pub verb: String,
}

/// The branch a `git push` names: the destination of its first refspec
/// (`HEAD:abc-1` → `abc-1`, `+main` → `main`). `None` for a bare `git push`
/// or `HEAD` — the current branch.
fn push_branch(args: &[&str]) -> Option<String> {
    let positional: Vec<&str> = args
        .iter()
        .copied()
        .filter(|a| !a.starts_with('-'))
        .collect();
    let refspec = positional.get(1)?.trim_start_matches('+');
    let dst = refspec.rsplit(':').next().unwrap_or(refspec);
    let dst = dst.strip_prefix("refs/heads/").unwrap_or(dst);
    (!dst.is_empty() && dst != "HEAD").then(|| dst.to_owned())
}

/// Every Bash command a run issued, in order.
#[must_use]
pub fn bash_commands(events: &[Event]) -> Vec<String> {
    let mut out = Vec::new();
    for event in events {
        let Some(payload) = event.payload.as_deref() else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<Value>(payload) else {
            continue;
        };
        let Some(blocks) = value.pointer("/message/content").and_then(Value::as_array) else {
            continue;
        };
        for block in blocks {
            if block.get("type").and_then(Value::as_str) == Some("tool_use")
                && block.get("name").and_then(Value::as_str) == Some("Bash")
            {
                if let Some(cmd) = block.pointer("/input/command").and_then(Value::as_str) {
                    out.push(cmd.to_owned());
                }
            }
        }
    }
    out
}

const GIT_WRITES: &[&str] = &[
    "push",
    "commit",
    "merge",
    "rebase",
    "reset",
    "worktree",
    "branch",
    "tag",
    "cherry-pick",
    "revert",
    "stash",
    "am",
];
const GH_WRITES: &[(&str, &[&str])] = &[
    (
        "pr",
        &[
            "create", "merge", "close", "reopen", "edit", "comment", "review", "ready",
        ],
    ),
    ("issue", &["create", "comment", "edit", "close", "reopen"]),
    ("run", &["rerun", "cancel"]),
    ("release", &["create", "edit", "delete"]),
    ("repo", &["create", "edit", "delete"]),
];

/// `owner/name` from a GitHub URL or an `owner/name` argument.
fn github_repo(token: &str) -> Option<String> {
    let rest = token
        .strip_prefix("https://github.com/")
        .unwrap_or(token)
        .trim_end_matches(".git");
    let mut parts = rest.split('/');
    let (owner, name) = (parts.next()?, parts.next()?);
    let ok = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || "-_.".contains(c));
    (ok(owner) && ok(name) && (token.starts_with("https://github.com/") || parts.next().is_none()))
        .then(|| format!("{owner}/{name}"))
}

/// The repo a `gh` command names: `-R`/`--repo`, a GitHub URL, or an API
/// path `repos/<owner>/<name>/…`.
fn named_repo(words: &[&str]) -> Option<String> {
    words
        .windows(2)
        .find(|w| matches!(w[0], "-R" | "--repo"))
        .and_then(|w| github_repo(w[1]))
        .or_else(|| {
            words.iter().find_map(|w| {
                w.starts_with("https://github.com/")
                    .then(|| github_repo(w))
                    .flatten()
            })
        })
        .or_else(|| {
            // gh api repos/<owner>/<name>/…
            words.iter().find_map(|w| {
                let rest = w.strip_prefix("repos/")?;
                github_repo(&rest.split('/').take(2).collect::<Vec<_>>().join("/"))
            })
        })
}

/// The writes in one shell command line (`&&`, `;`, `|` and newlines split
/// it into simple commands). `workdir` is where a command with no `-C` or
/// repo argument ran.
#[must_use]
pub fn writes_in(command: &str, workdir: &Path) -> Vec<Write> {
    let mut out = Vec::new();
    let simple = command.split(['\n', ';', '|']).flat_map(|s| s.split("&&"));
    for part in simple {
        let words: Vec<&str> = part
            .split_whitespace()
            .map(|w| w.trim_matches(|c| c == '"' || c == '\''))
            .collect();
        match words.first().copied() {
            Some("git") => {
                let mut dir = workdir.to_path_buf();
                let mut i = 1;
                while i < words.len() && words[i].starts_with('-') {
                    if words[i] == "-C" {
                        if let Some(p) = words.get(i + 1) {
                            dir = crate::paths::expand_tilde(Path::new(p))
                                .unwrap_or_else(|_| PathBuf::from(p));
                        }
                        i += 1;
                    }
                    i += 1;
                }
                if let Some(verb) = words.get(i).filter(|v| GIT_WRITES.contains(v)) {
                    // `worktree list`, `branch` alone, `stash list`, `tag`
                    // alone only read.
                    let next = words.get(i + 1).copied().unwrap_or("");
                    let reads = matches!(
                        (*verb, next),
                        ("worktree", "list")
                            | ("branch", "" | "--list" | "-a" | "-r" | "--show-current")
                            | ("stash", "list" | "show")
                            | ("tag", "" | "-l" | "--list")
                    );
                    if !reads {
                        let branch = (*verb == "push")
                            .then(|| push_branch(&words[i + 1..]))
                            .flatten();
                        let verb = if *verb == "worktree" {
                            format!("worktree {next}")
                        } else {
                            (*verb).to_owned()
                        };
                        out.push(Write {
                            target: Target::Local(dir),
                            branch,
                            verb,
                        });
                    }
                }
            }
            Some("gh") => {
                let (group, action) = (
                    words.get(1).copied().unwrap_or(""),
                    words.get(2).copied().unwrap_or(""),
                );
                let api_write = group == "api"
                    && (part.contains("mutation")
                        || words.windows(2).any(|w| {
                            matches!(w[0], "-X" | "--method") && !w[1].eq_ignore_ascii_case("GET")
                        })
                        || (!part.contains("graphql")
                            && words.iter().any(|w| {
                                matches!(*w, "-f" | "-F" | "--field" | "--raw-field" | "--input")
                            })));
                let listed = GH_WRITES
                    .iter()
                    .any(|(g, actions)| *g == group && actions.contains(&action));
                if !(api_write || listed) {
                    continue;
                }
                let named = named_repo(&words);
                let verb = if group == "api" {
                    "api write".to_owned()
                } else {
                    format!("{group} {action}")
                };
                let branch = (group == "pr")
                    .then(|| {
                        words
                            .windows(2)
                            .find(|w| matches!(w[0], "-H" | "--head"))
                            .map(|w| w[1].to_owned())
                    })
                    .flatten();
                out.push(Write {
                    target: named
                        .map_or_else(|| Target::Local(workdir.to_path_buf()), Target::GitHub),
                    branch,
                    verb,
                });
            }
            _ => {}
        }
    }
    out
}

/// Is this a push to one of the `protected` orgs' `main`/`master`?
#[must_use]
pub fn is_protected_main(repo: &str, branch: Option<&str>, protected: &[String]) -> bool {
    let org = repo.split('/').next().unwrap_or("");
    protected.iter().any(|p| p.eq_ignore_ascii_case(org))
        && matches!(branch, Some("main" | "master"))
}

/// One line for the run's log: each repo and branch (already resolved) with
/// its writes counted, e.g. `acme/widgets @ abc-1 (commit ×2, push)`.
/// Input: `(repo, branch, verb)`; `protected` are the orgs whose main a
/// push is flagged on.
#[must_use]
pub fn summary(
    writes: &[(String, Option<String>, String)],
    protected: &[String],
) -> Option<String> {
    let mut by_repo: BTreeMap<(&str, Option<&str>), BTreeMap<&str, usize>> = BTreeMap::new();
    for (repo, branch, verb) in writes {
        *by_repo
            .entry((repo, branch.as_deref()))
            .or_default()
            .entry(verb)
            .or_default() += 1;
    }
    if by_repo.is_empty() {
        return None;
    }
    let parts: Vec<String> = by_repo
        .into_iter()
        .map(|((repo, branch), verbs)| {
            let flagged = verbs.contains_key("push") && is_protected_main(repo, branch, protected);
            let verbs: Vec<String> = verbs
                .into_iter()
                .map(|(v, n)| {
                    if n > 1 {
                        format!("{v} ×{n}")
                    } else {
                        v.to_owned()
                    }
                })
                .collect();
            let at = branch.map(|b| format!(" @ {b}")).unwrap_or_default();
            let flag = if flagged { " ⚠ PUSHED TO MAIN" } else { "" };
            format!("{repo}{at} ({}){flag}", verbs.join(", "))
        })
        .collect();
    Some(format!("repos touched: {}", parts.join(" · ")))
}

#[cfg(test)]
// Tests should fail loudly on a broken fixture; unwrapping is the clearest way.
#[allow(clippy::unwrap_used)]
#[path = "tests/repotrace.rs"]
mod tests;
