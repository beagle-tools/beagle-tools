// Tests fail loudly on setup errors; unwrap/expect is the clearest way.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::{self, Cursor, Read, Write};
use std::time::Duration;

use super::*;
use crate::auth::Token;

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

/// An in-memory connection: reads from `input`, collects what is written.
struct Duplex {
    input: Cursor<Vec<u8>>,
    output: Vec<u8>,
}

impl Read for Duplex {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.input.read(buf)
    }
}

impl Write for Duplex {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.output.write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn send(raw: &str) -> (Option<String>, String) {
    let ctx = Ctx::new(
        Token::parse(TOKEN).unwrap(),
        crate::router::tests::unavailable(),
    );
    let mut conn = Duplex {
        input: Cursor::new(raw.as_bytes().to_vec()),
        output: Vec::new(),
    };
    let line = exchange(&mut conn, &ctx, "100.64.0.9");
    (line, String::from_utf8(conn.output).unwrap())
}

#[test]
fn health_over_the_wire() {
    let (line, out) = send("GET /v1/health HTTP/1.1\r\nHost: x\r\n\r\n");
    assert!(out.starts_with("HTTP/1.1 200 OK\r\n"), "{out}");
    assert!(out.ends_with("{\"ok\":true}"));
    let line = line.unwrap();
    assert!(line.contains(" 100.64.0.9 GET /v1/health 200 "), "{line}");
}

#[test]
fn version_without_a_token_is_401_over_the_wire() {
    let (_, out) = send("GET /v1/version HTTP/1.1\r\n\r\n");
    assert!(out.starts_with("HTTP/1.1 401 Unauthorized\r\n"), "{out}");
    assert!(out.contains("WWW-Authenticate: Bearer\r\n"));
}

#[test]
fn version_with_a_token_over_the_wire() {
    let (_, out) = send(&format!(
        "GET /v1/version HTTP/1.1\r\nAuthorization: Bearer {TOKEN}\r\n\r\n"
    ));
    assert!(out.starts_with("HTTP/1.1 200 OK\r\n"), "{out}");
}

#[test]
fn head_gets_headers_only() {
    let (_, out) = send("HEAD /v1/health HTTP/1.1\r\n\r\n");
    assert!(out.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(out.ends_with("\r\n\r\n"));
}

#[test]
fn garbage_is_400_and_silence_is_nothing() {
    let (line, out) = send("hello\r\n\r\n");
    assert!(out.starts_with("HTTP/1.1 400 Bad Request\r\n"), "{out}");
    assert!(line.unwrap().contains(" - - 400 "));
    let (line, out) = send("");
    assert_eq!(line, None);
    assert_eq!(out, "");
}

#[test]
fn the_log_line_never_includes_the_query() {
    let req = Request::get("/v1/rca?token=secret", Some("Bearer secret"));
    let line = log_line("T", "p", Some(&req), 200, Duration::from_millis(3));
    assert_eq!(line, "T p GET /v1/rca 200 3ms");
}
