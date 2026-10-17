//! Tests for [`crate::ReviewStore`] — split out to honor the 400-line
//! file limit.

#![allow(clippy::unwrap_used)]

use crate::store::Cache;
use crate::ReviewStore;
use tandem_core::github::RepoRef;
use tandem_core::review::{CommentAuthorKind, CommentSeverity, DiffSide};
use tandem_core::review::{CommentStatus, NewLocalComment};

fn new_comment(repo: &RepoRef) -> NewLocalComment {
    NewLocalComment {
        repo: repo.clone(),
        pr_number: 1,
        head_sha: "abc".into(),
        path: "src/main.rs".into(),
        side: DiffSide::New,
        line: 3,
        body: "consider a match here".into(),
        author_kind: CommentAuthorKind::Agent,
        author_name: "claude".into(),
        severity: CommentSeverity::Suggestion,
        run_id: Some("run-1".into()),
        tag: Some("coding-standards".into()),
        parent_id: None,
        end_line: None,
        suggestion: None,
        github_comment_id: None,
    }
}

#[test]
fn comment_lifecycle() {
    let cache = Cache::open_in_memory().unwrap();
    let repo = RepoRef::parse("o/r").unwrap();

    let c = cache.add_comment(new_comment(&repo)).unwrap();
    assert_eq!(c.status, CommentStatus::Open);

    cache
        .set_comment_status(&c.id, CommentStatus::Accepted)
        .unwrap();
    let listed = cache.list_comments(&repo, 1).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].status, CommentStatus::Accepted);

    cache.delete_comment(&c.id).unwrap();
    assert!(cache.list_comments(&repo, 1).unwrap().is_empty());
}

#[test]
fn deleting_a_root_promotes_the_oldest_reply() {
    let cache = Cache::open_in_memory().unwrap();
    let repo = RepoRef::parse("o/r").unwrap();

    let root = cache.add_comment(new_comment(&repo)).unwrap();
    let mut reply = new_comment(&repo);
    reply.parent_id = Some(root.id.clone());
    let first = cache.add_comment(reply.clone()).unwrap();
    let second = cache.add_comment(reply).unwrap();

    cache.delete_comment(&root.id).unwrap();
    let listed = cache.list_comments(&repo, 1).unwrap();
    assert_eq!(listed.len(), 2);
    let new_root = listed.iter().find(|c| c.id == first.id).unwrap();
    assert_eq!(new_root.parent_id, None);
    let adopted = listed.iter().find(|c| c.id == second.id).unwrap();
    assert_eq!(adopted.parent_id, Some(first.id.clone()));
}

#[test]
fn missing_comment_errors() {
    let cache = Cache::open_in_memory().unwrap();
    assert!(cache
        .set_comment_status("nope", CommentStatus::Resolved)
        .is_err());
}
