// Tests fail loudly on setup errors; unwrap/expect is the clearest way.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use serde_json::json;

use super::*;

pub(crate) const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn ctx() -> Ctx {
    Ctx::new(Token::parse(TOKEN).unwrap(), unavailable())
}

/// Stores that all fail to resolve, for tests that never reach one.
pub(crate) fn unavailable() -> Stores {
    Stores {
        scout: Err("none in tests".into()),
        rca: Err("none in tests".into()),
        recap: Err("none in tests".into()),
        standup: Err("none in tests".into()),
    }
}

fn bearer() -> String {
    format!("Bearer {TOKEN}")
}

#[test]
fn health_needs_no_token() {
    let r = handle(&ctx(), &Request::get("/v1/health", None));
    assert_eq!(r.status, 200);
    assert_eq!(r.body, json!({"ok": true}));
}

#[test]
fn everything_else_needs_the_token() {
    for path in ["/v1/version", "/v1/nope", "/", "/v1/health/x"] {
        let r = handle(&ctx(), &Request::get(path, None));
        assert_eq!(r.status, 401, "{path}");
        assert_eq!(r.body["error"]["code"], "unauthorized");
    }
}

#[test]
fn a_wrong_token_is_401() {
    let wrong = format!("Bearer {}", TOKEN.replace('f', "e"));
    let r = handle(&ctx(), &Request::get("/v1/version", Some(&wrong)));
    assert_eq!(r.status, 401);
}

#[test]
fn version_with_the_token() {
    let r = handle(&ctx(), &Request::get("/v1/version", Some(&bearer())));
    assert_eq!(r.status, 200);
    assert_eq!(r.body["api"], 1);
    assert_eq!(r.body["beagle"], env!("CARGO_PKG_VERSION"));
    assert_eq!(r.body["tools"][0], "scout");
}

#[test]
fn unknown_paths_are_404_once_authenticated() {
    let r = handle(&ctx(), &Request::get("/v1/nope", Some(&bearer())));
    assert_eq!(r.status, 404);
    assert_eq!(r.body["error"]["code"], "not_found");
}

#[test]
fn writes_are_405_even_without_a_token() {
    let mut req = Request::get("/v1/version", None);
    req.method = Method::Other("POST".into());
    let r = handle(&ctx(), &req);
    assert_eq!(r.status, 405);
    assert_eq!(r.body["error"]["code"], "method_not_allowed");
}

#[test]
fn an_unresolved_store_is_500_with_the_reason() {
    let r = handle(&ctx(), &Request::get("/v1/scout/projects", Some(&bearer())));
    assert_eq!(r.status, 500);
    assert_eq!(
        r.body["error"]["message"],
        "scout store unavailable: none in tests"
    );
}
