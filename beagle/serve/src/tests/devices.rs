// Tests fail loudly on setup errors; unwrap/expect is the clearest way.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use serde_json::{json, Value};

use crate::auth::Token;
use crate::http::{Method, Request, Response};
use crate::push::devices::{DeviceStore, MAX_DEVICES};
use crate::router::tests::{unavailable, TOKEN};
use crate::router::{handle, Ctx};

const ID: &str = "9b1deb4d-3b7d-4bad-9bdd-2b0d7b3dcb6d";

struct Env {
    _tmp: tempfile::TempDir,
    store: Arc<DeviceStore>,
    ctx: Ctx,
}

fn env() -> Env {
    let tmp = tempfile::tempdir().unwrap();
    let store = Arc::new(DeviceStore::new(tmp.path().join("state/devices.json")));
    let ctx =
        Ctx::new(Token::parse(TOKEN).unwrap(), unavailable()).with_devices(Arc::clone(&store));
    Env {
        _tmp: tmp,
        store,
        ctx,
    }
}

fn call(env: &Env, method: Method, path: &str, body: Option<&Value>) -> Response {
    let bearer = format!("Bearer {TOKEN}");
    handle(&env.ctx, &Request::new(method, path, Some(&bearer), body))
}

fn registration() -> Value {
    json!({ "apns_token": "AB".repeat(32), "environment": "development",
            "topic": "dev.beagletools.beagle", "name": "iPhone" })
}

#[test]
fn registering_creates_then_refreshes_a_phone_with_default_settings() {
    let env = env();
    let r = call(
        &env,
        Method::Put,
        &format!("/v1/devices/{ID}"),
        Some(&registration()),
    );
    assert_eq!(r.status, 201, "{}", r.body);
    assert_eq!(r.body["settings"]["scout"]["events"]["blocked"], true);
    assert_eq!(r.body["settings"]["scout"]["events"]["task_done"], false);
    assert_eq!(r.body["catalog"][0]["tool"], "scout");
    assert_eq!(r.body["catalog"][0]["events"][0]["kind"], "blocked");

    let r = call(
        &env,
        Method::Put,
        &format!("/v1/devices/{ID}"),
        Some(&registration()),
    );
    assert_eq!(r.status, 200);
    let saved = env.store.load().unwrap();
    let device = &saved.devices[ID];
    assert_eq!(device.apns_token.as_deref(), Some("ab".repeat(32).as_str()));
    assert_eq!(device.name, "iPhone");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(env.store.path())
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}

#[test]
fn settings_round_trip_and_are_validated() {
    let env = env();
    let path = format!("/v1/devices/{ID}/notifications");
    assert_eq!(call(&env, Method::Get, &path, None).status, 404);
    call(
        &env,
        Method::Put,
        &format!("/v1/devices/{ID}"),
        Some(&registration()),
    );

    let wanted =
        json!({ "scout": { "enabled": true, "events": { "task_done": true, "blocked": false } } });
    let r = call(&env, Method::Put, &path, Some(&wanted));
    assert_eq!(r.status, 200, "{}", r.body);
    let r = call(&env, Method::Get, &path, None);
    assert_eq!(r.body["settings"]["scout"]["events"]["task_done"], true);
    assert_eq!(r.body["settings"]["scout"]["events"]["blocked"], false);
    assert_eq!(r.body["settings"]["scout"]["events"]["project_done"], true);

    let bad = json!({ "scout": { "enabled": true, "events": { "nope": true } } });
    assert_eq!(call(&env, Method::Put, &path, Some(&bad)).status, 400);
}

#[test]
fn bad_requests_are_refused() {
    let env = env();
    let mut bad = registration();
    bad["apns_token"] = json!("xyz");
    assert_eq!(
        call(&env, Method::Put, &format!("/v1/devices/{ID}"), Some(&bad)).status,
        400
    );
    let mut extra = registration();
    extra["extra"] = json!(1);
    assert_eq!(
        call(
            &env,
            Method::Put,
            &format!("/v1/devices/{ID}"),
            Some(&extra)
        )
        .status,
        400
    );
    assert_eq!(
        call(
            &env,
            Method::Put,
            "/v1/devices/not-a-uuid",
            Some(&registration())
        )
        .status,
        400
    );

    // Not declared JSON.
    let bearer = format!("Bearer {TOKEN}");
    let mut req = Request::new(
        Method::Put,
        &format!("/v1/devices/{ID}"),
        Some(&bearer),
        Some(&registration()),
    );
    req.content_type = Some("text/plain".into());
    assert_eq!(handle(&env.ctx, &req).status, 415);

    // No token, no write.
    let req = Request::new(
        Method::Put,
        &format!("/v1/devices/{ID}"),
        None,
        Some(&registration()),
    );
    assert_eq!(handle(&env.ctx, &req).status, 401);
    assert!(env.store.load().unwrap().devices.is_empty());
}

#[test]
fn methods_that_write_only_reach_the_device_api() {
    let env = env();
    for path in ["/v1/scout/projects", "/v1/version", "/v1/health"] {
        let r = call(&env, Method::Put, path, Some(&json!({})));
        assert_eq!(r.status, 405, "{path}");
    }
    let r = call(&env, Method::Get, &format!("/v1/devices/{ID}"), None);
    assert_eq!(r.status, 405);
    assert!(r.headers.contains(&("Allow", "PUT, DELETE".to_owned())));
    assert_eq!(call(&env, Method::Get, "/v1/devices", None).status, 404);
}

#[test]
fn delete_forgets_a_phone_and_test_says_push_is_off() {
    let env = env();
    call(
        &env,
        Method::Put,
        &format!("/v1/devices/{ID}"),
        Some(&registration()),
    );
    let r = call(&env, Method::Post, &format!("/v1/devices/{ID}/test"), None);
    assert_eq!(
        (r.status, r.body["error"]["code"].clone()),
        (503, json!("push_not_configured"))
    );
    assert_eq!(
        call(&env, Method::Delete, &format!("/v1/devices/{ID}"), None).status,
        204
    );
    assert_eq!(
        call(&env, Method::Delete, &format!("/v1/devices/{ID}"), None).status,
        204
    );
    assert_eq!(
        call(&env, Method::Post, &format!("/v1/devices/{ID}/test"), None).status,
        404
    );
}

#[test]
fn at_most_ten_phones() {
    let env = env();
    for n in 0..MAX_DEVICES {
        let id = format!("00000000-0000-0000-0000-{n:012}");
        assert_eq!(
            call(
                &env,
                Method::Put,
                &format!("/v1/devices/{id}"),
                Some(&registration())
            )
            .status,
            201
        );
    }
    let r = call(
        &env,
        Method::Put,
        &format!("/v1/devices/{ID}"),
        Some(&registration()),
    );
    assert_eq!(
        (r.status, r.body["error"]["code"].clone()),
        (409, json!("too_many_devices"))
    );
}
