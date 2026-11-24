//! Tests for the write half of the GitHub client: each user action sends
//! exactly the request GitHub expects. A one-shot fake server on
//! 127.0.0.1 records what arrives and answers with a canned response.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // panicking is the correct failure mode in tests

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::mpsc::{self, Receiver};

use base64::Engine as _;
use serde_json::{json, Value};
use tandem_core::github::RepoRef;
use tandem_core::TandemError;

use super::NewInlineComment;
use crate::auth::{GithubAuth, GithubConfig};
use crate::client::GithubClient;

/// One request as the fake server saw it.
#[derive(Debug)]
struct Seen {
    method: String,
    path: String,
    auth: Option<String>,
    body: Value,
}

/// Serves `responses` in order, one connection each, and reports every
/// request it read. Returns the API base to point a client at.
fn serve(responses: Vec<(u16, String)>) -> (String, Receiver<Seen>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for (status, body) in responses {
            let Ok((stream, _)) = listener.accept() else {
                return;
            };
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut request_line = String::new();
            reader.read_line(&mut request_line).unwrap();
            let mut parts = request_line.split_whitespace();
            let method = parts.next().unwrap_or_default().to_owned();
            let path = parts.next().unwrap_or_default().to_owned();
            let (mut length, mut auth) = (0usize, None);
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let line = line.trim_end();
                if line.is_empty() {
                    break;
                }
                let (name, value) = line.split_once(':').unwrap_or((line, ""));
                match name.to_ascii_lowercase().as_str() {
                    "content-length" => length = value.trim().parse().unwrap_or(0),
                    "authorization" => auth = Some(value.trim().to_owned()),
                    _ => {}
                }
            }
            let mut raw = vec![0; length];
            reader.read_exact(&mut raw).unwrap();
            let body_seen = serde_json::from_slice(&raw).unwrap_or(Value::Null);
            tx.send(Seen {
                method,
                path,
                auth,
                body: body_seen,
            })
            .ok();
            let mut stream = stream;
            write!(
                stream,
                "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\n\
                 Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        }
    });
    (base, rx)
}

async fn client(base: &str) -> GithubClient {
    GithubClient::connect(&GithubConfig {
        auth: GithubAuth::Token {
            token: "t0ken".to_owned(),
        },
        api_base: base.to_owned(),
    })
    .await
    .unwrap()
}

fn repo() -> RepoRef {
    RepoRef::parse("acme/widgets").unwrap()
}

fn ok(body: &Value) -> (u16, String) {
    (200, body.to_string())
}

#[tokio::test]
async fn an_issue_comment_posts_the_body_with_the_token() {
    let (base, seen) = serve(vec![ok(&json!({ "id": 42 }))]);
    let id = client(&base)
        .await
        .post_issue_comment(&repo(), 7, "looks good")
        .await
        .unwrap();
    assert_eq!(id, 42);
    let req = seen.recv().unwrap();
    assert_eq!(req.method, "POST");
    assert_eq!(req.path, "/repos/acme/widgets/issues/7/comments");
    assert_eq!(req.auth.as_deref(), Some("Bearer t0ken"));
    assert_eq!(req.body, json!({ "body": "looks good" }));
}

#[tokio::test]
async fn a_reply_goes_to_the_thread_it_answers() {
    let (base, seen) = serve(vec![ok(&json!({ "id": 5 }))]);
    client(&base)
        .await
        .reply_to_review_comment(&repo(), 7, 99, "fixed")
        .await
        .unwrap();
    let req = seen.recv().unwrap();
    assert_eq!(req.path, "/repos/acme/widgets/pulls/7/comments/99/replies");
    assert_eq!(req.body, json!({ "body": "fixed" }));
}

#[tokio::test]
async fn an_inline_comment_anchors_to_commit_path_line_and_side() {
    let (base, seen) = serve(vec![ok(&json!({ "id": 1 })), ok(&json!({ "id": 2 }))]);
    let client = client(&base).await;
    for right_side in [true, false] {
        let new = NewInlineComment {
            commit_id: "abc123",
            path: "src/lib.rs",
            line: 12,
            right_side,
            body: "nit",
        };
        client.create_review_comment(&repo(), 7, new).await.unwrap();
        let req = seen.recv().unwrap();
        assert_eq!(req.path, "/repos/acme/widgets/pulls/7/comments");
        assert_eq!(
            req.body,
            json!({
                "body": "nit",
                "commit_id": "abc123",
                "path": "src/lib.rs",
                "line": 12,
                "side": if right_side { "RIGHT" } else { "LEFT" },
            })
        );
    }
}

#[tokio::test]
async fn approving_sends_the_approve_event_and_an_optional_body() {
    let (base, seen) = serve(vec![ok(&json!({})), ok(&json!({}))]);
    let client = client(&base).await;
    client.approve_pull_request(&repo(), 7, None).await.unwrap();
    let without_body = seen.recv().unwrap();
    assert_eq!(without_body.path, "/repos/acme/widgets/pulls/7/reviews");
    assert_eq!(without_body.body, json!({ "event": "APPROVE" }));
    client
        .approve_pull_request(&repo(), 7, Some("ship it"))
        .await
        .unwrap();
    assert_eq!(
        seen.recv().unwrap().body,
        json!({ "event": "APPROVE", "body": "ship it" })
    );
}

#[tokio::test]
async fn a_response_without_an_id_is_an_error() {
    let (base, _seen) = serve(vec![ok(&json!({ "node_id": "x" }))]);
    let err = client(&base)
        .await
        .post_issue_comment(&repo(), 7, "hi")
        .await
        .unwrap_err();
    assert!(err.to_string().contains("missing id"), "{err}");
}

#[tokio::test]
async fn committing_a_suggestion_puts_base64_content_on_the_branch() {
    let (base, seen) = serve(vec![ok(&json!({ "commit": { "sha": "c0ffee" } }))]);
    let sha = client(&base)
        .await
        .commit_file(
            &repo(),
            "src/a.rs",
            "feat",
            "apply suggestion",
            "fn a() {}\n",
            "blob1",
        )
        .await
        .unwrap();
    assert_eq!(sha, "c0ffee");
    let req = seen.recv().unwrap();
    assert_eq!(req.method, "PUT");
    assert_eq!(req.path, "/repos/acme/widgets/contents/src/a.rs");
    let content = base64::engine::general_purpose::STANDARD.encode("fn a() {}\n");
    assert_eq!(
        req.body,
        json!({
            "message": "apply suggestion",
            "content": content,
            "sha": "blob1",
            "branch": "feat",
        })
    );
}

#[tokio::test]
async fn merge_sends_the_method_and_surfaces_githubs_refusal() {
    let (base, seen) = serve(vec![
        ok(&json!({ "merged": true })),
        (
            405,
            r#"{"message":"Pull Request is not mergeable"}"#.to_owned(),
        ),
    ]);
    let client = client(&base).await;
    client
        .merge_pull_request(&repo(), 7, "squash")
        .await
        .unwrap();
    let req = seen.recv().unwrap();
    assert_eq!(
        (req.method.as_str(), req.path.as_str()),
        ("PUT", "/repos/acme/widgets/pulls/7/merge")
    );
    assert_eq!(req.body, json!({ "merge_method": "squash" }));
    let err = client
        .merge_pull_request(&repo(), 7, "merge")
        .await
        .unwrap_err();
    match err {
        TandemError::GithubApi { status, message } => {
            assert_eq!(status, 405);
            assert!(message.contains("not mergeable"), "{message}");
        }
        other => panic!("unexpected error: {other}"),
    }
}

#[tokio::test]
async fn update_branch_puts_to_the_update_branch_endpoint() {
    let (base, seen) = serve(vec![ok(&json!({}))]);
    client(&base).await.update_branch(&repo(), 7).await.unwrap();
    let req = seen.recv().unwrap();
    assert_eq!(req.method, "PUT");
    assert_eq!(req.path, "/repos/acme/widgets/pulls/7/update-branch");
}

#[tokio::test]
async fn graphql_mutations_send_their_variables() {
    let (base, seen) = serve(vec![ok(&json!({ "data": {} })); 3]);
    let client = client(&base).await;
    client.resolve_review_thread("PRRT_1").await.unwrap();
    let resolve = seen.recv().unwrap();
    assert_eq!(resolve.path, "/graphql");
    assert!(resolve.body["query"]
        .as_str()
        .unwrap()
        .contains("resolveReviewThread"));
    assert_eq!(resolve.body["variables"], json!({ "id": "PRRT_1" }));

    client.enable_auto_merge("PR_9", "squash").await.unwrap();
    let auto = seen.recv().unwrap();
    assert_eq!(
        auto.body["variables"],
        json!({ "id": "PR_9", "method": "SQUASH" })
    );

    client.mark_pr_ready_for_review("PR_9").await.unwrap();
    let ready = seen.recv().unwrap();
    assert!(ready.body["query"]
        .as_str()
        .unwrap()
        .contains("markPullRequestReadyForReview"));
}

#[tokio::test]
async fn a_graphql_error_in_a_200_is_still_an_error() {
    let body = json!({ "errors": [{ "message": "Resource not accessible" }] });
    let (base, _seen) = serve(vec![ok(&body)]);
    let err = client(&base)
        .await
        .resolve_review_thread("PRRT_1")
        .await
        .unwrap_err();
    assert!(err.to_string().contains("Resource not accessible"), "{err}");
}

#[tokio::test]
async fn a_plain_403_is_not_retried() {
    // One canned answer only: a retry would hang on accept, then fail on
    // the closed server instead of returning GitHub's 403.
    let (base, seen) = serve(vec![(403, r#"{"message":"no access"}"#.to_owned())]);
    let err = client(&base)
        .await
        .post_issue_comment(&repo(), 7, "hi")
        .await
        .unwrap_err();
    assert!(
        matches!(err, TandemError::GithubApi { status: 403, .. }),
        "{err}"
    );
    assert!(seen.recv().is_ok());
    assert!(seen.try_recv().is_err(), "exactly one request");
}
