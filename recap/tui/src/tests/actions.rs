//! Tests for `actions` — planning only; nothing here touches GitHub.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::*;

fn check(
    state: PrState,
    review: Option<Review>,
    checks: Checks,
    mergeable: &str,
    merge_state: &str,
) -> MergeCheck {
    MergeCheck {
        state,
        review,
        checks,
        mergeable: mergeable.to_owned(),
        merge_state: merge_state.to_owned(),
        head: "abc123".to_owned(),
    }
}

fn squash_only() -> RepoSettings {
    parse_repo_settings(r#"{"squashMergeAllowed":true,"mergeCommitAllowed":false,"rebaseMergeAllowed":false,"viewerDefaultMergeMethod":"SQUASH"}"#).unwrap()
}

const GREEN: Checks = Checks {
    passed: 5,
    failed: 0,
    pending: 0,
};

#[test]
fn repo_settings_put_the_default_first() {
    assert_eq!(squash_only().methods, [MergeMethod::Squash]);
    let all = parse_repo_settings(r#"{"squashMergeAllowed":true,"mergeCommitAllowed":true,"rebaseMergeAllowed":true,"viewerDefaultMergeMethod":"REBASE"}"#).unwrap();
    assert_eq!(
        all.methods,
        [MergeMethod::Rebase, MergeMethod::Squash, MergeMethod::Merge]
    );
    assert!(parse_repo_settings("nope").is_none());
}

#[test]
fn clean_approved_green_merges_now_against_the_checked_commit() {
    let plan = merge_plan(
        &check(
            PrState::Open,
            Some(Review::Approved),
            GREEN,
            "MERGEABLE",
            "CLEAN",
        ),
        &squash_only(),
    )
    .unwrap();
    assert!(!plan.auto);
    assert_eq!(
        merge_args("u", MergeMethod::Squash, &plan),
        [
            "pr",
            "merge",
            "u",
            "--squash",
            "--match-head-commit",
            "abc123"
        ]
    );
}

#[test]
fn running_checks_or_missing_approval_offer_auto_merge() {
    let pending = Checks {
        passed: 3,
        failed: 0,
        pending: 2,
    };
    let plan = merge_plan(
        &check(
            PrState::Open,
            Some(Review::Approved),
            pending,
            "MERGEABLE",
            "BLOCKED",
        ),
        &squash_only(),
    )
    .unwrap();
    assert!(plan.auto);
    assert!(merge_args("u", MergeMethod::Squash, &plan).contains(&"--auto".to_owned()));
    let needs_review = merge_plan(
        &check(
            PrState::Open,
            Some(Review::Required),
            GREEN,
            "MERGEABLE",
            "BLOCKED",
        ),
        &squash_only(),
    )
    .unwrap();
    assert!(needs_review.auto);
}

#[test]
fn refuses_with_a_plain_reason() {
    let r = squash_only();
    let reason = |c: MergeCheck| merge_plan(&c, &r).unwrap_err();
    assert!(
        reason(check(PrState::Draft, None, GREEN, "MERGEABLE", "DRAFT")).contains("mark it ready")
    );
    assert!(reason(check(PrState::Merged, None, GREEN, "", "")).contains("already merged"));
    assert!(reason(check(
        PrState::Open,
        Some(Review::Approved),
        GREEN,
        "CONFLICTING",
        "DIRTY"
    ))
    .contains("conflicts"));
    assert!(reason(check(
        PrState::Open,
        Some(Review::ChangesRequested),
        GREEN,
        "MERGEABLE",
        "BLOCKED"
    ))
    .contains("requested changes"));
    let failing = Checks {
        passed: 4,
        failed: 2,
        pending: 1,
    };
    assert_eq!(
        reason(check(
            PrState::Open,
            Some(Review::Approved),
            failing,
            "MERGEABLE",
            "UNSTABLE"
        )),
        "2 check(s) failing"
    );
    assert!(reason(check(
        PrState::Open,
        Some(Review::Approved),
        GREEN,
        "MERGEABLE",
        "BEHIND"
    ))
    .contains("behind"));
    assert!(reason(check(
        PrState::Open,
        Some(Review::Approved),
        GREEN,
        "MERGEABLE",
        "BLOCKED"
    ))
    .contains("branch protection"));
    assert!(reason(check(PrState::Open, None, GREEN, "UNKNOWN", "UNKNOWN")).contains("try again"));
    let none = RepoSettings {
        methods: Vec::new(),
    };
    assert!(merge_plan(
        &check(
            PrState::Open,
            Some(Review::Approved),
            GREEN,
            "MERGEABLE",
            "CLEAN"
        ),
        &none
    )
    .is_err());
}

#[test]
fn parses_a_real_gh_merge_check() {
    let json = r#"{"state":"OPEN","isDraft":false,"reviewDecision":"APPROVED","mergeable":"MERGEABLE",
      "mergeStateStatus":"BEHIND","headRefOid":"deadbeef",
      "statusCheckRollup":[{"__typename":"CheckRun","status":"COMPLETED","conclusion":"SUCCESS"}]}"#;
    let c = parse_merge_check(json).unwrap();
    assert_eq!(
        (c.state, c.merge_state.as_str(), c.head.as_str()),
        (PrState::Open, "BEHIND", "deadbeef")
    );
    assert_eq!(c.checks.passed, 1);
}
