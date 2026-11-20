//! Tests for the local comment state (`comments/local.rs`), the review
//! queue (`sync::pending`) and the GraphQL parsing edge cases.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use std::path::Path;

use super::local::write_json;
use super::*;
use crate::model::{PrRef, PrState, RecapId, RecapManifest};
use crate::store::Store;

const AUTHOR: &str = "alice";

fn said(id: &str, author: &str, created: &str, body: &str) -> Comment {
    Comment {
        id: id.to_owned(),
        author: author.to_owned(),
        bot: false,
        context: false,
        created: created.to_owned(),
        body: body.to_owned(),
        url: format!("https://github.com/acme/widgets/pull/1#{id}"),
    }
}

fn inline(id: &str, comments: Vec<Comment>) -> Thread {
    Thread {
        id: id.to_owned(),
        kind: ThreadKind::Inline {
            path: "src/lib.rs".to_owned(),
            line: Some(3),
            resolved: false,
            outdated: false,
        },
        comments,
    }
}

fn write_threads(recap_dir: &Path, threads: Vec<Thread>) {
    write_json(
        &dir(recap_dir).join("threads.json"),
        &Threads {
            fetched: "0".to_owned(),
            threads,
        },
    )
    .unwrap();
}

fn asked() -> Thread {
    inline(
        "t-1",
        vec![
            said("1", "bob", "2026-11-01T10:00:00Z", "Why a loop here?"),
            said("2", AUTHOR, "2026-11-01T10:05:00Z", "Looking."),
            said("3", "bob", "2026-11-01T10:10:00Z", "Any news?"),
        ],
    )
}

#[test]
fn a_thread_is_unread_until_marked_and_again_after_a_new_comment() {
    let tmp = tempfile::tempdir().unwrap();
    let mut thread = asked();
    assert!(!is_read(&thread, &load_read(tmp.path())));

    let mut marks = ReadMarks::new();
    marks.insert("t-1".to_owned(), "3".to_owned());
    save_read(tmp.path(), &marks).unwrap();
    assert!(is_read(&thread, &load_read(tmp.path())));

    thread
        .comments
        .push(said("4", "carol", "2026-11-01T11:00:00Z", "+1"));
    assert!(!is_read(&thread, &load_read(tmp.path())));
}

#[test]
fn a_review_covers_the_newest_comment_from_someone_else() {
    let tmp = tempfile::tempdir().unwrap();
    write_threads(tmp.path(), vec![asked()]);

    save_review(tmp.path(), "t-1", AUTHOR, "  ## In short\nAnswer bob.\n").unwrap();
    let review = load_review(tmp.path(), "t-1").unwrap();
    assert_eq!(review.covers, "3");
    assert_eq!(review.body, "## In short\nAnswer bob.");
    assert!(is_reviewed(tmp.path(), &asked(), AUTHOR));

    // A newer comment from someone else makes the review stale.
    let mut newer = asked();
    newer
        .comments
        .push(said("5", "carol", "2026-11-01T12:00:00Z", "And this?"));
    assert!(!is_reviewed(tmp.path(), &newer, AUTHOR));
    // The author's own reply does not.
    let mut answered = asked();
    answered
        .comments
        .push(said("6", AUTHOR, "2026-11-01T12:00:00Z", "Done."));
    assert!(is_reviewed(tmp.path(), &answered, AUTHOR));
}

#[test]
fn reviewing_an_unknown_thread_points_at_sync() {
    let tmp = tempfile::tempdir().unwrap();
    let err = save_review(tmp.path(), "t-9", AUTHOR, "x").unwrap_err();
    assert!(err.to_string().contains("comments sync"), "{err}");
}

#[test]
fn missing_or_corrupt_local_files_read_as_empty() {
    let tmp = tempfile::tempdir().unwrap();
    assert_eq!(load(tmp.path()).threads, Vec::<Thread>::new());
    std::fs::create_dir_all(dir(tmp.path())).unwrap();
    std::fs::write(dir(tmp.path()).join("threads.json"), "{not json").unwrap();
    std::fs::write(dir(tmp.path()).join("read.json"), "[1,2").unwrap();
    assert_eq!(load(tmp.path()).threads, Vec::<Thread>::new());
    assert!(load_read(tmp.path()).is_empty());
    assert!(load_review(tmp.path(), "t-1").is_none());
}

fn manifest(number: u64, state: PrState) -> RecapManifest {
    RecapManifest {
        repo: "acme/widgets".to_owned(),
        number,
        title: format!("ABC-{number}: change"),
        url: format!("https://github.com/acme/widgets/pull/{number}"),
        state,
        author: Some(AUTHOR.to_owned()),
        branch: None,
        base: None,
        ticket: None,
        ticket_url: None,
        additions: 0,
        deletions: 0,
        files: 0,
        created: Some("2026-11-01T00:00:00Z".to_owned()),
        updated: None,
        merged_at: None,
    }
}

fn recap(store: &Store, number: u64, state: PrState, threads: Vec<Thread>) -> RecapId {
    let id = RecapId::for_pr(&PrRef::parse(&format!("acme/widgets#{number}")).unwrap());
    let recap_dir = store.create(&id, &manifest(number, state), "").unwrap();
    write_threads(&recap_dir, threads);
    id
}

#[test]
fn pending_lists_only_settled_unanswered_threads_on_open_prs() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::new(tmp.path());
    let now = unix_of("2026-11-01T12:00:00Z").unwrap();

    let open = recap(&store, 1, PrState::Open, vec![asked()]);
    recap(&store, 2, PrState::Merged, vec![asked()]);
    let fresh = inline(
        "t-2",
        vec![said("7", "bob", "2026-11-01T11:59:00Z", "Quick one?")],
    );
    recap(&store, 3, PrState::Draft, vec![fresh]);

    let got = pending(&store, now).unwrap();
    assert_eq!(
        got.len(),
        1,
        "merged PRs and settling threads wait: {got:?}"
    );
    assert_eq!(got[0].recap, open.to_string());
    assert_eq!(got[0].thread, "t-1");
    assert_eq!(got[0].comment, "3");
    assert_eq!(got[0].author, "bob");

    // Once reviewed, it leaves the queue.
    save_review(&store.recap_dir(&open), "t-1", AUTHOR, "done").unwrap();
    assert_eq!(pending(&store, now).unwrap(), Vec::<Pending>::new());
}

#[test]
fn a_response_of_the_wrong_shape_is_an_error() {
    let config = CommentsConfig::default();
    assert!(parse_threads("not json", &config).is_err());
    assert!(parse_threads(r#"{"data":{"repository":null}}"#, &config).is_err());
}

#[test]
fn an_outdated_inline_thread_keeps_its_original_line() {
    let json = r#"{"data":{"repository":{"pullRequest":{
      "reviewThreads":{"nodes":[
        {"isResolved":false,"isOutdated":true,"path":"a.rs","line":null,"originalLine":9,
         "comments":{"nodes":[
           {"databaseId":1,"author":{"login":"bob","__typename":"User"},"body":"hm","createdAt":"2026-11-01T10:00:00Z","url":"u"}]}}]},
      "comments":{"nodes":[]},
      "reviews":{"nodes":[]}
    }}}}"#;
    let threads = parse_threads(json, &CommentsConfig::default()).unwrap();
    assert_eq!(threads.len(), 1);
    assert_eq!(
        threads[0].kind,
        ThreadKind::Inline {
            path: "a.rs".to_owned(),
            line: Some(9),
            resolved: false,
            outdated: true,
        }
    );
}
