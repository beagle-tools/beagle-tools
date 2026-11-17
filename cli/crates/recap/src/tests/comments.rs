//! Tests for `comments`.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::local::{forget_gone, write_json};
use super::*;

/// A GraphQL response: one inline thread a bot started and a person
/// answered, one bot-only thread, a person's conversation comment, a bare
/// approval, and a review with words.
const RESPONSE: &str = r#"{"data":{"repository":{"pullRequest":{
  "reviewThreads":{"nodes":[
    {"isResolved":false,"isOutdated":false,"path":"helm/hpa.yaml","line":12,"originalLine":12,
     "comments":{"nodes":[
       {"databaseId":1,"author":{"login":"review-bot","__typename":"Bot"},"body":"Consider an HPA.","createdAt":"2026-11-01T10:00:00Z","url":"u1"},
       {"databaseId":2,"author":{"login":"bob","__typename":"User"},"body":"HPAs are not needed for our ingress.","createdAt":"2026-11-01T11:00:00Z","url":"u2"}]}},
    {"isResolved":true,"isOutdated":true,"path":"a.go","line":null,"originalLine":3,
     "comments":{"nodes":[
       {"databaseId":3,"author":{"login":"lint-bot","__typename":"Bot"},"body":"bug?","createdAt":"2026-11-01T09:00:00Z","url":"u3"}]}}]},
  "comments":{"nodes":[
    {"databaseId":4,"author":{"login":"carol","__typename":"User"},"body":"Why the new port?","createdAt":"2026-11-01T12:00:00Z","url":"u4"},
    {"databaseId":5,"author":{"login":"tracker-bot","__typename":"Bot"},"body":"ABC-1","createdAt":"2026-11-01T08:00:00Z","url":"u5"}]},
  "reviews":{"nodes":[
    {"databaseId":6,"author":{"login":"brandonbirdj","__typename":"User"},"body":"","state":"APPROVED","submittedAt":"2026-11-01T13:00:00Z","url":"u6"},
    {"databaseId":7,"author":{"login":"brandonbirdj","__typename":"User"},"body":"One question inline.","state":"COMMENTED","submittedAt":"2026-11-01T14:00:00Z","url":"u7"}]}
}}}}"#;

fn threads() -> Vec<Thread> {
    parse_threads(RESPONSE, &CommentsConfig::default()).unwrap()
}

#[test]
fn bots_are_context_and_bot_only_threads_are_dropped() {
    let t = threads();
    let ids: Vec<&str> = t.iter().map(|t| t.id.as_str()).collect();
    // The bot-only thread, the tracker linkback and the bare approval are gone.
    assert_eq!(ids, vec!["t-1", "c-4", "r-7"]);
    let inline = &t[0];
    assert_eq!(inline.place(), "helm/hpa.yaml:12");
    // The bot's opener stays, as context, with its [bot] name restored.
    assert_eq!(inline.comments[0].author, "review-bot[bot]");
    assert!(inline.comments[0].context);
    assert!(!inline.comments[1].context);
    assert_eq!(inline.last_counted().unwrap().id, "2");
}

#[test]
fn the_ignore_list_is_configurable() {
    let config = CommentsConfig {
        ignore_authors: vec!["carol".to_owned()],
        ignore_bots: false,
        automated_markers: Vec::new(),
    };
    let t = parse_threads(RESPONSE, &config).unwrap();
    // Bots count now; carol does not.
    assert!(t.iter().any(|t| t.id == "t-3"));
    assert!(!t.iter().any(|t| t.id == "c-4"));
}

#[test]
fn automated_markers_are_configurable_and_off_by_default() {
    let body = "Fixed in abc123.\n\n_Automated reply_";
    assert!(!CommentsConfig::default().is_automated(body));
    let config: CommentsConfig =
        toml::from_str("automated_markers = [\"Automated reply\", \"\"]").unwrap();
    assert!(config.is_automated(body));
    assert!(!config.is_automated("a person wrote this"));
    assert!(config.ignore_bots, "unset keys keep their defaults");
}

#[test]
fn reading_and_reviewing_track_the_newest_comment() {
    let tmp = tempfile::tempdir().unwrap();
    let recap = tmp.path();
    write_json(
        &dir(recap).join("threads.json"),
        &Threads {
            fetched: "now".to_owned(),
            threads: threads(),
        },
    )
    .unwrap();
    let t = &load(recap).threads[0];

    let mut read = load_read(recap);
    assert!(!is_read(t, &read));
    read.insert(t.id.clone(), "2".to_owned());
    save_read(recap, &read).unwrap();
    assert!(is_read(t, &load_read(recap)));

    // alice is the PR author: bob's comment needs an answer.
    assert!(!is_reviewed(recap, t, "alice"));
    save_review(
        recap,
        "t-1",
        "alice",
        "## In short\nMostly valid.\n\n**Valid?** Mostly.",
    )
    .unwrap();
    assert!(is_reviewed(recap, t, "alice"));

    let m = marks(recap, "alice", unix_of("2026-11-02T00:00:00Z").unwrap());
    assert_eq!((m.total, m.unread, m.reviewed, m.pending), (3, 2, 1, 2));

    // A thread where only the author spoke needs no review.
    assert!(is_reviewed(recap, t, "bob"));
}

#[test]
fn only_threads_where_someone_else_spoke_are_kept() {
    // As bob's own PR, the thread where only bob (and a bot) spoke goes.
    let kept = from_others(threads(), "bob");
    let ids: Vec<&str> = kept.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(ids, vec!["c-4", "r-7"]);
}

#[test]
fn resolved_threads_are_dropped_with_their_local_review() {
    let tmp = tempfile::tempdir().unwrap();
    let recap = tmp.path();
    let all = threads();
    let mut resolved = all.clone();
    if let ThreadKind::Inline { resolved, .. } = &mut resolved[0].kind {
        *resolved = true;
    }
    write_json(
        &dir(recap).join("threads.json"),
        &Threads {
            fetched: "0".to_owned(),
            threads: all,
        },
    )
    .unwrap();
    save_review(recap, "t-1", "alice", "old").unwrap();
    let mut read = ReadMarks::new();
    read.insert("t-1".to_owned(), "2".to_owned());
    save_read(recap, &read).unwrap();

    let open = open_only(resolved);
    assert!(!open.iter().any(|t| t.id == "t-1"));
    forget_gone(recap, &open);
    assert!(load_review(recap, "t-1").is_none());
    assert!(load_read(recap).is_empty());
}

#[test]
fn a_review_splits_into_its_short_summary_and_the_rest() {
    let review = Review {
        covers: "2".to_owned(),
        body: "## In short\n- bob says the HPA is redundant.\n\n## What they're saying\nMore."
            .to_owned(),
    };
    let (short, rest) = review.split();
    assert_eq!(short.as_deref(), Some("- bob says the HPA is redundant."));
    assert_eq!(rest, "## What they're saying\nMore.");
    let plain = Review {
        covers: "2".to_owned(),
        body: "No sections.".to_owned(),
    };
    assert_eq!(plain.split(), (None, "No sections.".to_owned()));
}

#[test]
fn only_what_is_waiting_on_matt_and_settled_needs_an_agent() {
    let tmp = tempfile::tempdir().unwrap();
    let recap = tmp.path();
    let t = threads();
    write_json(
        &dir(recap).join("threads.json"),
        &Threads {
            fetched: "0".to_owned(),
            threads: t.clone(),
        },
    )
    .unwrap();
    let later = unix_of("2026-11-01T11:30:00Z").unwrap();
    // bob's comment (11:00) is 30 minutes old: review it.
    assert_eq!(need(recap, &t[0], "alice", later), Need::Review);
    // Five minutes after it: let the burst settle.
    let soon = unix_of("2026-11-01T11:05:00Z").unwrap();
    assert_eq!(need(recap, &t[0], "alice", soon), Need::Settling);
    // Alice answered last: their turn, not his.
    let mut answered = t[0].clone();
    answered.comments.push(Comment {
        id: "9".to_owned(),
        author: "alice".to_owned(),
        bot: false,
        context: false,
        created: "2026-11-01T11:10:00Z".to_owned(),
        body: "done, take another look?".to_owned(),
        url: String::new(),
    });
    assert_eq!(need(recap, &answered, "alice", later), Need::TheirTurn);
    // Reviewed: nothing to do.
    save_review(recap, "t-1", "alice", "## In short\nok").unwrap();
    assert_eq!(need(recap, &t[0], "alice", later), Need::Reviewed);
    // An approval with words but no question is informational.
    let approval = Thread {
        id: "r-6".to_owned(),
        kind: ThreadKind::Review {
            state: "APPROVED".to_owned(),
        },
        comments: vec![Comment {
            body: "LGTM, nice.".to_owned(),
            ..t[1].comments[0].clone()
        }],
    };
    assert_eq!(need(recap, &approval, "alice", later), Need::Nothing);
}

#[test]
fn github_timestamps_parse_to_unix_seconds() {
    assert_eq!(unix_of("1970-01-01T00:00:00Z"), Some(0));
    assert_eq!(unix_of("2026-11-01T11:00:00Z"), Some(1_793_530_800));
    assert_eq!(unix_of("not a date"), None);
}

#[test]
fn rate_limits_are_recognised() {
    assert!(is_rate_limit("API rate limit exceeded for user"));
    assert!(is_rate_limit("You have exceeded a secondary rate limit"));
    assert!(!is_rate_limit("Could not resolve to a Repository"));
}
