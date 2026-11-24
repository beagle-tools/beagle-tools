// Tests fail loudly on setup errors; unwrap/expect/panic is the clearest way.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::Cursor;

use super::*;

fn read(raw: &str) -> Result<Request, ReadError> {
    read_request(&mut Cursor::new(raw.as_bytes().to_vec()))
}

#[test]
fn parses_a_get_with_query_and_authorization() {
    let req = read(
        "GET /v1/rca?archived=true&x=1 HTTP/1.1\r\nHost: h\r\nauthorization: Bearer abc\r\n\r\n",
    )
    .unwrap();
    assert_eq!(req.method, Method::Get);
    assert_eq!(req.path, "/v1/rca");
    assert_eq!(req.param("archived"), Some("true"));
    assert_eq!(req.param("x"), Some("1"));
    assert_eq!(req.param("y"), None);
    assert_eq!(req.authorization.as_deref(), Some("Bearer abc"));
}

#[test]
fn head_and_other_methods_are_recognised() {
    assert_eq!(
        read("HEAD / HTTP/1.0\r\n\r\n").unwrap().method,
        Method::Head
    );
    assert_eq!(
        read("POST /v1/x HTTP/1.1\r\n\r\n").unwrap().method,
        Method::Other("POST".into())
    );
}

#[test]
fn an_incomplete_head_is_closed_not_answered() {
    assert!(matches!(
        read("GET / HTTP/1.1\r\nHost: h\r\n"),
        Err(ReadError::Closed)
    ));
    assert!(matches!(read(""), Err(ReadError::Closed)));
}

#[test]
fn malformed_heads_are_rejected() {
    for raw in [
        "GET\r\n\r\n",
        "GET / HTTP/2\r\n\r\n",
        "GET http://x/ HTTP/1.1\r\n\r\n",
        "GET / HTTP/1.1 extra\r\n\r\n",
        "GET / HTTP/1.1\r\nno-colon\r\n\r\n",
    ] {
        assert!(matches!(read(raw), Err(ReadError::Malformed(_))), "{raw:?}");
    }
}

#[test]
fn an_oversized_head_is_too_large() {
    let raw = format!(
        "GET / HTTP/1.1\r\nX: {}\r\n\r\n",
        "a".repeat(MAX_HEAD_BYTES)
    );
    assert!(matches!(read(&raw), Err(ReadError::TooLarge)));
    let endless = format!("GET / HTTP/1.1\r\nX: {}", "a".repeat(3 * MAX_HEAD_BYTES));
    assert!(matches!(read(&endless), Err(ReadError::TooLarge)));
}

#[test]
fn responses_carry_length_and_close() {
    let mut out = Vec::new();
    Response::ok(json!({"ok": true}))
        .write_to(&mut out, false)
        .unwrap();
    let text = String::from_utf8(out).unwrap();
    assert!(text.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(text.contains("Content-Length: 11\r\n"));
    assert!(text.contains("Connection: close\r\n"));
    assert!(text.ends_with("\r\n\r\n{\"ok\":true}"));
}

#[test]
fn head_responses_have_no_body() {
    let mut out = Vec::new();
    Response::ok(json!({"ok": true}))
        .write_to(&mut out, true)
        .unwrap();
    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("Content-Length: 11\r\n"));
    assert!(text.ends_with("\r\n\r\n"));
}

#[test]
fn errors_have_the_documented_body_and_headers() {
    let r = Response::error(401, "unauthorized", "no");
    assert_eq!(
        r.body,
        json!({"error": {"code": "unauthorized", "message": "no"}})
    );
    assert!(r
        .headers
        .contains(&("WWW-Authenticate", "Bearer".to_owned())));
    let r = Response::error(405, "method_not_allowed", "no");
    assert!(r.headers.contains(&("Allow", "GET, HEAD".to_owned())));
}
