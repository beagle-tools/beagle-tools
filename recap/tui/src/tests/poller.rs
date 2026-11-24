//! Tests for `poller` (parsing only — no network).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::*;
use crate::model::PrState;

#[test]
fn open_approved_with_mixed_checks() {
    let json = r#"{"state":"OPEN","isDraft":false,"reviewDecision":"APPROVED","updatedAt":"2026-10-28T15:00:00Z",
      "statusCheckRollup":[
        {"__typename":"CheckRun","status":"COMPLETED","conclusion":"SUCCESS"},
        {"__typename":"CheckRun","status":"COMPLETED","conclusion":"SKIPPED"},
        {"__typename":"CheckRun","status":"COMPLETED","conclusion":"FAILURE"},
        {"__typename":"CheckRun","status":"IN_PROGRESS","conclusion":""},
        {"__typename":"StatusContext","state":"SUCCESS"},
        {"__typename":"StatusContext","state":"PENDING"}
      ]}"#;
    let s = parse_status_json(json).unwrap();
    assert_eq!(s.state, PrState::Open);
    assert_eq!(s.review, Some(Review::Approved));
    assert_eq!(
        s.checks,
        Checks {
            passed: 3,
            failed: 1,
            pending: 2
        }
    );
    assert_eq!(s.updated.as_deref(), Some("2026-10-28T15:00:00Z"));
}

#[test]
fn draft_merged_closed_and_garbage() {
    let draft =
        parse_status_json(r#"{"state":"OPEN","isDraft":true,"reviewDecision":"REVIEW_REQUIRED"}"#)
            .unwrap();
    assert_eq!(
        (draft.state, draft.review),
        (PrState::Draft, Some(Review::Required))
    );
    assert!(!draft.checks.any());
    assert_eq!(
        parse_status_json(r#"{"state":"MERGED"}"#).unwrap().state,
        PrState::Merged
    );
    let closed = parse_status_json(r#"{"state":"CLOSED","reviewDecision":""}"#).unwrap();
    assert_eq!((closed.state, closed.review), (PrState::Closed, None));
    assert!(parse_status_json(r#"{"state":"WEIRD"}"#).is_none());
    assert!(parse_status_json("nope").is_none());
}
