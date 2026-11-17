//! TUI tests: the Comments tab.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::tests::{fixture, id, key};
use super::*;
use crate::wrap;
use crossterm::event::KeyCode;
use std::fs;

#[test]
fn the_comments_tab_reads_threads_as_you_rest_on_them() {
    use crate::comments::{self, Comment, Thread, ThreadKind, Threads};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    let (_tmp, store) = fixture();
    let dir = store.recap_dir(&id("o/newer#2"));
    let comment = |id: &str, author: &str, body: &str| Comment {
        id: id.to_owned(),
        author: author.to_owned(),
        bot: false,
        context: false,
        created: "2026-11-01T10:00:00Z".to_owned(),
        body: body.to_owned(),
        url: String::new(),
    };
    let threads = Threads {
        fetched: "0".to_owned(),
        threads: vec![
            Thread {
                id: "t-1".to_owned(),
                kind: ThreadKind::Inline {
                    path: "src/server.rs".to_owned(),
                    line: Some(12),
                    resolved: false,
                    outdated: false,
                },
                comments: vec![comment("1", "bob", "This retry loop never gives up.")],
            },
            Thread {
                id: "c-2".to_owned(),
                kind: ThreadKind::Conversation,
                comments: vec![comment("2", "carol", "Why the new port?")],
            },
        ],
    };
    fs::create_dir_all(dir.join("comments")).unwrap();
    fs::write(
        dir.join("comments/threads.json"),
        serde_json::to_string(&threads).unwrap(),
    )
    .unwrap();
    comments::save_review(
        &dir,
        "t-1",
        "",
        "## In short\nbob thinks the retry loop needs a limit.\n\n**Valid.** It retries forever on a 500.",
    )
    .unwrap();

    let mut app = App::new(store, None).unwrap();
    let marks = |app: &App| app.comment_marks["newer-2"];
    assert_eq!(
        (
            marks(&app).unread,
            marks(&app).reviewed,
            marks(&app).pending
        ),
        (2, 1, 1)
    );

    let mut t = Terminal::new(TestBackend::new(140, 30)).unwrap();
    key(&mut app, KeyCode::Char('3'));
    assert_eq!(app.tab, Tab::Comments);
    t.draw(|f| view::draw(f, &mut app)).unwrap();
    let screen: String = t
        .backend()
        .buffer()
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect();
    assert!(screen.contains("3 Comments"));
    assert!(screen.contains("src/server.rs:12"));
    assert!(screen.contains("agent review"));
    // Resting on the first thread read it.
    assert_eq!(marks(&app).unread, 1);

    // n switches to the next thread's sub-tab, and shows only it.
    key(&mut app, KeyCode::Char('n'));
    assert_eq!(app.status.as_deref(), Some("comment 2 of 2"));
    let shown: String = app.view.doc.iter().map(wrap::plain_text).collect();
    assert!(shown.contains("Why the new port?"));
    assert!(!shown.contains("This retry loop never gives up"));
    t.draw(|f| view::draw(f, &mut app)).unwrap();
    assert_eq!(marks(&app).unread, 0);

    key(&mut app, KeyCode::Char('u'));
    assert_eq!(marks(&app).unread, 1);
    // Read marks are saved, so they survive a restart.
    assert_eq!(comments::load_read(&dir).len(), 1);

    // tab reaches the agent review pane on the Comments tab; j scrolls it.
    assert_eq!(app.focus, Focus::Content);
    key(&mut app, KeyCode::Tab);
    assert_eq!(app.focus, Focus::Review);
    key(&mut app, KeyCode::Char('j'));
    assert_eq!(app.review_scroll, 1);
    key(&mut app, KeyCode::Tab);
    assert_eq!(app.focus, Focus::List);
    // Leaving the Comments tab never strands focus on the review pane.
    key(&mut app, KeyCode::Tab);
    key(&mut app, KeyCode::Tab);
    key(&mut app, KeyCode::Char('1'));
    assert_eq!(app.focus, Focus::Content);
}
