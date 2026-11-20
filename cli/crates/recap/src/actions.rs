//! Acting on a PR from the TUI: mark it ready (or back to draft), merge it,
//! or close it — always through `gh`, always after re-reading its live state.
//!
//! The decision of *whether* a PR can be merged, and how, is a pure function
//! ([`merge_plan`]) over GitHub's own fields, so it's unit tested without the
//! network. The functions that change GitHub ([`set_ready`], [`merge`],
//! [`close`]) run only when the user confirms in the TUI.

use serde::Deserialize;

use crate::model::PrState;
use crate::poller::{self, Checks, Review};

/// How a PR is merged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeMethod {
    /// One squashed commit.
    Squash,
    /// A merge commit.
    Merge,
    /// Commits rebased onto the base branch.
    Rebase,
}

impl MergeMethod {
    /// The `gh pr merge` flag.
    #[must_use]
    pub fn flag(self) -> &'static str {
        match self {
            Self::Squash => "--squash",
            Self::Merge => "--merge",
            Self::Rebase => "--rebase",
        }
    }

    /// Lowercase label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Squash => "squash",
            Self::Merge => "merge commit",
            Self::Rebase => "rebase",
        }
    }
}

/// Which merge methods a repo allows, and the user's default.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoSettings {
    /// Allowed methods, the default first.
    pub methods: Vec<MergeMethod>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawRepo {
    #[serde(default)]
    squash_merge_allowed: bool,
    #[serde(default)]
    merge_commit_allowed: bool,
    #[serde(default)]
    rebase_merge_allowed: bool,
    #[serde(default)]
    viewer_default_merge_method: Option<String>,
}

/// Parses `gh repo view --json squashMergeAllowed,mergeCommitAllowed,rebaseMergeAllowed,viewerDefaultMergeMethod`.
#[must_use]
pub fn parse_repo_settings(json: &str) -> Option<RepoSettings> {
    let raw: RawRepo = serde_json::from_str(json).ok()?;
    let default = match raw.viewer_default_merge_method.as_deref() {
        Some("SQUASH") => Some(MergeMethod::Squash),
        Some("MERGE") => Some(MergeMethod::Merge),
        Some("REBASE") => Some(MergeMethod::Rebase),
        _ => None,
    };
    let mut methods: Vec<MergeMethod> = [
        (MergeMethod::Squash, raw.squash_merge_allowed),
        (MergeMethod::Merge, raw.merge_commit_allowed),
        (MergeMethod::Rebase, raw.rebase_merge_allowed),
    ]
    .into_iter()
    .filter_map(|(m, ok)| ok.then_some(m))
    .collect();
    if let Some(d) = default.filter(|d| methods.contains(d)) {
        methods.retain(|m| *m != d);
        methods.insert(0, d);
    }
    Some(RepoSettings { methods })
}

/// A PR's live mergeability, as GitHub reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeCheck {
    /// The PR's state.
    pub state: PrState,
    /// Review decision.
    pub review: Option<Review>,
    /// CI checks.
    pub checks: Checks,
    /// `mergeable`: `MERGEABLE`, `CONFLICTING` or `UNKNOWN`.
    pub mergeable: String,
    /// `mergeStateStatus`: `CLEAN`, `BLOCKED`, `BEHIND`, `DIRTY`, `UNSTABLE`,
    /// `HAS_HOOKS`, `DRAFT` or `UNKNOWN`.
    pub merge_state: String,
    /// The head commit the check was made against.
    pub head: String,
}

/// The `--json` fields a merge check reads.
const CHECK_FIELDS: &str =
    "state,isDraft,reviewDecision,statusCheckRollup,mergeable,mergeStateStatus,headRefOid";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawMergeability {
    #[serde(default)]
    mergeable: String,
    #[serde(default)]
    merge_state_status: String,
    #[serde(default)]
    head_ref_oid: String,
}

/// Parses `gh pr view --json state,isDraft,reviewDecision,statusCheckRollup,mergeable,mergeStateStatus,headRefOid`.
#[must_use]
pub fn parse_merge_check(json: &str) -> Option<MergeCheck> {
    let status = poller::parse_status_json(json)?;
    let raw: RawMergeability = serde_json::from_str(json).ok()?;
    Some(MergeCheck {
        state: status.state,
        review: status.review,
        checks: status.checks,
        mergeable: raw.mergeable,
        merge_state: raw.merge_state_status,
        head: raw.head_ref_oid,
    })
}

/// What a confirmed merge will do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergePlan {
    /// Methods to offer, the default first.
    pub methods: Vec<MergeMethod>,
    /// `true`: checks are still running, so enable auto-merge (GitHub merges
    /// once every requirement is met) instead of merging now.
    pub auto: bool,
    /// Merge only if the PR's head is still this commit.
    pub head: String,
}

/// Decides whether a PR can be merged, and how. `Err` is a plain-English
/// reason for the status line.
///
/// # Errors
/// A reason whenever the PR shouldn't be merged right now: not open,
/// conflicts, changes requested, failing checks, behind its base branch,
/// blocked by branch protection, or no allowed merge method.
pub fn merge_plan(check: &MergeCheck, repo: &RepoSettings) -> Result<MergePlan, String> {
    match check.state {
        PrState::Draft => return Err("it's a draft — mark it ready first (R)".to_owned()),
        PrState::Merged => return Err("it's already merged".to_owned()),
        PrState::Closed => return Err("it's closed".to_owned()),
        PrState::Open => {}
    }
    if repo.methods.is_empty() {
        return Err("this repo allows no merge method you can use".to_owned());
    }
    if check.mergeable == "CONFLICTING" || check.merge_state == "DIRTY" {
        return Err("it has merge conflicts with its base branch".to_owned());
    }
    if check.review == Some(Review::ChangesRequested) {
        return Err("a reviewer requested changes".to_owned());
    }
    if check.checks.failed > 0 {
        return Err(format!("{} check(s) failing", check.checks.failed));
    }
    if check.merge_state == "BEHIND" {
        return Err("its branch is behind the base branch — update it first".to_owned());
    }
    let plan = |auto| {
        Ok(MergePlan {
            methods: repo.methods.clone(),
            auto,
            head: check.head.clone(),
        })
    };
    let ready = matches!(check.merge_state.as_str(), "CLEAN" | "HAS_HOOKS");
    if ready && check.checks.pending == 0 {
        return plan(false);
    }
    // Not mergeable yet, but nothing is wrong: checks still running or an
    // approval still to come. Auto-merge lets GitHub finish the job.
    if check.checks.pending > 0 || check.review == Some(Review::Required) {
        return plan(true);
    }
    Err(match check.merge_state.as_str() {
        "BLOCKED" => {
            "blocked by branch protection (a required review or check is missing)".to_owned()
        }
        "UNKNOWN" | "" => {
            "GitHub is still working out if it can merge — try again in a moment".to_owned()
        }
        other => format!("GitHub reports it as {other}"),
    })
}

fn gh(args: &[&str]) -> Result<String, String> {
    let out = crate::gh::run_gh(args, crate::gh::GH_TIMEOUT).map_err(|e| format!("gh: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_owned())
    }
}

/// Reads a PR's live mergeability and its repo's merge settings, and plans
/// the merge (read-only).
///
/// # Errors
/// `gh` failures or a reason from [`merge_plan`].
pub fn check_merge(url: &str, repo: &str) -> Result<MergePlan, String> {
    let pr = gh(&["pr", "view", url, "--json", CHECK_FIELDS])?;
    let check = parse_merge_check(&pr).ok_or("couldn't read the PR's state")?;
    let settings = gh(&[
        "repo",
        "view",
        repo,
        "--json",
        "squashMergeAllowed,mergeCommitAllowed,rebaseMergeAllowed,viewerDefaultMergeMethod",
    ])?;
    let settings =
        parse_repo_settings(&settings).ok_or("couldn't read the repo's merge settings")?;
    merge_plan(&check, &settings)
}

/// The `gh pr merge` arguments for a confirmed plan.
#[must_use]
pub fn merge_args(url: &str, method: MergeMethod, plan: &MergePlan) -> Vec<String> {
    let mut args = vec![
        "pr".to_owned(),
        "merge".to_owned(),
        url.to_owned(),
        method.flag().to_owned(),
    ];
    if plan.auto {
        args.push("--auto".to_owned());
    }
    if !plan.head.is_empty() {
        args.push("--match-head-commit".to_owned());
        args.push(plan.head.clone());
    }
    args
}

/// Merges (or enables auto-merge on) a PR. **Changes GitHub.**
///
/// # Errors
/// `gh`'s error, verbatim.
pub fn merge(url: &str, method: MergeMethod, plan: &MergePlan) -> Result<String, String> {
    let args = merge_args(url, method, plan);
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    gh(&refs).map(|_| {
        if plan.auto {
            format!(
                "auto-merge ({}) enabled — GitHub merges it once checks pass",
                method.label()
            )
        } else {
            format!("merged ({})", method.label())
        }
    })
}

/// Marks a draft ready for review, or (`undo`) an open PR back to draft.
/// **Changes GitHub.**
///
/// # Errors
/// `gh`'s error, verbatim.
pub fn set_ready(url: &str, undo: bool) -> Result<String, String> {
    let mut args = vec!["pr", "ready", url];
    if undo {
        args.push("--undo");
    }
    gh(&args).map(|_| {
        if undo {
            "moved back to draft"
        } else {
            "marked ready for review"
        }
        .to_owned()
    })
}

/// Closes a PR without merging. **Changes GitHub.**
///
/// # Errors
/// `gh`'s error, verbatim.
pub fn close(url: &str) -> Result<String, String> {
    gh(&["pr", "close", url]).map(|_| "closed".to_owned())
}

#[cfg(test)]
#[path = "tests/actions.rs"]
mod tests;
