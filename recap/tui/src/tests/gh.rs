//! Tests for `gh`.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::*;

#[test]
fn gh_json_becomes_a_manifest() {
    let pr = PrRef::parse("acme/gadgets#1590").unwrap();
    let json = r#"{"number":1590,"title":"ABC-754: Raise the cache size to 2",
        "url":"https://github.com/acme/gadgets/pull/1590","state":"OPEN","isDraft":true,
        "author":{"login":"octocat"},"headRefName":"abc-754","baseRefName":"main",
        "additions":1,"deletions":1,"changedFiles":1,"createdAt":"2026-10-27T19:39:00Z","updatedAt":"2026-10-28T14:58:00Z"}"#;
    let m = manifest_from_json(&pr, json).unwrap();
    assert_eq!(m.repo, "acme/gadgets");
    assert_eq!(m.state, PrState::Draft);
    assert_eq!(m.ticket.as_deref(), Some("ABC-754"));
    assert_eq!(m.author.as_deref(), Some("octocat"));
    assert_eq!((m.additions, m.deletions, m.files), (1, 1, 1));
}

#[test]
fn merged_and_minimal_json() {
    let pr = PrRef::parse("o/r#2").unwrap();
    let m = manifest_from_json(
        &pr,
        r#"{"number":2,"title":"chore","url":"u","state":"MERGED"}"#,
    )
    .unwrap();
    assert_eq!(m.state, PrState::Merged);
    assert_eq!(m.ticket, None);
    assert!(manifest_from_json(&pr, "not json").is_err());
    assert!(
        manifest_from_json(&pr, r#"{"number":2,"title":"t","url":"u","state":"WEIRD"}"#).is_err()
    );
}

#[test]
fn comments_include_inline_review_threads() {
    let discussion = r#"{"reviews":[
        {"author":{"login":"review-bot"},"state":"APPROVED","body":"Approved.\nThree claims unverified."},
        {"author":{"login":"brandonbirdj"},"state":"COMMENTED","body":""}],
      "comments":[{"author":{"login":"tracker-bot"},"body":"ABC-696 link"}]}"#;
    let inline = r#"[
        {"user":{"login":"brandonbirdj"},"path":"Makefile","line":317,"body":"My experience with --server-side=true is that it's difficult to delete objects.","in_reply_to_id":null},
        {"user":{"login":"alice"},"path":"Makefile","line":null,"original_line":317,"body":"Checking.","in_reply_to_id":1}]"#;
    let text = format_comments(discussion, inline).unwrap();
    assert!(text.contains("- review-bot APPROVED: Approved.\n  Three claims unverified."));
    assert!(text.contains("- brandonbirdj COMMENTED\n"));
    assert!(text.contains("- brandonbirdj on Makefile:317: My experience with --server-side=true"));
    assert!(text.contains("  ↳ reply alice on Makefile:317: Checking."));
    assert!(text.contains("## Conversation (1)\n- tracker-bot: ABC-696 link"));
    assert!(format_comments("{}", "nope").is_err());
}
