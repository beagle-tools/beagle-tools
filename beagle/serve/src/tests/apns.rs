// Tests fail loudly on setup errors; unwrap/expect is the clearest way.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::VecDeque;
use std::path::Path;
use std::sync::{Arc, Mutex};

use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::Engine as _;
use ring::rand::SystemRandom;
use ring::signature::{
    EcdsaKeyPair, UnparsedPublicKey, ECDSA_P256_SHA256_FIXED, ECDSA_P256_SHA256_FIXED_SIGNING,
};
use serde_json::Value;

use super::*;
use crate::config::PushConfig;
use crate::push::devices::Device;
use crate::push::{cmd, send_test, settings, SharedSender};

const PHONE: &str = "aaaaaaaa-0000-0000-0000-000000000001";

/// A fresh P-256 key as a `.p8` file's text.
fn p8() -> String {
    let doc = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &SystemRandom::new())
        .unwrap();
    let b64 = STANDARD.encode(doc.as_ref());
    let lines: Vec<&str> = b64
        .as_bytes()
        .chunks(64)
        .map(|c| std::str::from_utf8(c).unwrap())
        .collect();
    format!(
        "-----BEGIN PRIVATE KEY-----\n{}\n-----END PRIVATE KEY-----\n",
        lines.join("\n")
    )
}

fn signer() -> Signer {
    Signer::new(&p8(), "ABC123DEFG", "KEY1234567").unwrap()
}

/// Checks `jwt` is ES256, for the test key ids, signed by `public_key`.
fn verify(jwt: &str, public_key: &[u8]) -> Value {
    let parts: Vec<&str> = jwt.split('.').collect();
    assert_eq!(parts.len(), 3, "{jwt}");
    let header: Value = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(parts[0]).unwrap()).unwrap();
    assert_eq!(
        header,
        serde_json::json!({"alg": "ES256", "kid": "KEY1234567"})
    );
    let signature = URL_SAFE_NO_PAD.decode(parts[2]).unwrap();
    UnparsedPublicKey::new(&ECDSA_P256_SHA256_FIXED, public_key)
        .verify(format!("{}.{}", parts[0], parts[1]).as_bytes(), &signature)
        .expect("signature verifies");
    let claims: Value = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(parts[1]).unwrap()).unwrap();
    assert_eq!(claims["iss"], "ABC123DEFG");
    claims
}

fn phone(environment: Environment) -> Device {
    Device {
        apns_token: Some("ab".repeat(32)),
        environment,
        topic: "dev.beagletools.beagle".into(),
        name: "iPhone".into(),
        registered: "2026-11-25T00:00:00Z".into(),
        last_seen: "2026-11-25T00:00:00Z".into(),
        settings: settings::defaults(),
    }
}

fn store_with(dir: &Path, environment: Environment) -> Arc<DeviceStore> {
    let store = Arc::new(DeviceStore::new(dir.join("devices.json")));
    store
        .update(|d| d.devices.insert(PHONE.into(), phone(environment)))
        .unwrap();
    store
}

/// One request the fake APNs received.
#[derive(Debug, Clone)]
struct Seen {
    path: String,
    headers: http::HeaderMap,
    body: Vec<u8>,
}

/// A fake APNs: HTTP/2 (prior knowledge) on loopback, answering from a
/// script (then 200s) and recording every request.
struct FakeApns {
    url: String,
    seen: Arc<Mutex<Vec<Seen>>>,
}

fn fake_apns(script: Vec<(u16, &str)>) -> FakeApns {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    listener.set_nonblocking(true).unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let answers: Arc<Mutex<VecDeque<(u16, String)>>> = Arc::new(Mutex::new(
        script.into_iter().map(|(s, r)| (s, r.to_owned())).collect(),
    ));
    let (seen2, answers2) = (Arc::clone(&seen), answers);
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async move {
            let listener = tokio::net::TcpListener::from_std(listener).unwrap();
            loop {
                let Ok((socket, _)) = listener.accept().await else {
                    return;
                };
                let (seen, answers) = (Arc::clone(&seen2), Arc::clone(&answers2));
                tokio::spawn(async move {
                    let Ok(mut conn) = h2::server::handshake(socket).await else {
                        return;
                    };
                    while let Some(Ok((req, mut respond))) = conn.accept().await {
                        let (seen, answers) = (Arc::clone(&seen), Arc::clone(&answers));
                        tokio::spawn(async move {
                            let (parts, mut body) = req.into_parts();
                            let mut data = Vec::new();
                            while let Some(Ok(chunk)) = body.data().await {
                                let _ = body.flow_control().release_capacity(chunk.len());
                                data.extend_from_slice(&chunk);
                            }
                            seen.lock().unwrap().push(Seen {
                                path: parts.uri.path().to_owned(),
                                headers: parts.headers,
                                body: data,
                            });
                            let (status, reason) = answers
                                .lock()
                                .unwrap()
                                .pop_front()
                                .unwrap_or((200, String::new()));
                            let response =
                                http::Response::builder().status(status).body(()).unwrap();
                            let Ok(mut stream) = respond.send_response(response, reason.is_empty())
                            else {
                                return;
                            };
                            if !reason.is_empty() {
                                let body = serde_json::json!({ "reason": reason }).to_string();
                                let _ = stream.send_data(bytes::Bytes::from(body), true);
                            }
                        });
                    }
                });
            }
        });
    });
    FakeApns { url, seen }
}

fn pending(kind: &str, body: &str) -> Pending {
    let mut p = crate::push::test_pending(PHONE, &phone(Environment::Development)).unwrap();
    p.record.tool = "scout".into();
    p.record.kind = kind.into();
    p.record.project = "rate-limit-the-api".into();
    p.record.body = body.into();
    p.record.link = "beagle://scout/projects/rate-limit-the-api".into();
    p
}

#[test]
fn the_token_is_es256_signed_and_reused_for_fifty_minutes() {
    let mut s = signer();
    let first = s.token(1_000_000).unwrap();
    let claims = verify(&first, s.public_key());
    assert_eq!(claims["iat"], 1_000_000);
    assert_eq!(s.token(1_000_000 + 49 * 60).unwrap(), first);
    let later = s.token(1_000_000 + 50 * 60).unwrap();
    assert_ne!(later, first);
    s.expire();
    assert_ne!(s.token(1_000_000 + 50 * 60).unwrap(), later);
    assert!(Signer::new("not a key", "ABC123DEFG", "KEY1234567").is_err());
}

#[test]
fn payload_and_headers_follow_the_design() {
    let blocked: Value = serde_json::from_slice(&payload(&pending("blocked", "Need you"))).unwrap();
    assert_eq!(blocked["aps"]["alert"]["body"], "Need you");
    assert_eq!(blocked["aps"]["thread-id"], "scout-rate-limit-the-api");
    assert_eq!(blocked["aps"]["sound"], "default");
    assert_eq!(
        blocked["beagle"]["link"],
        "beagle://scout/projects/rate-limit-the-api"
    );
    let done: Value = serde_json::from_slice(&payload(&pending("task_done", "x"))).unwrap();
    assert!(done["aps"].get("sound").is_none());

    let headers = headers(&pending("task_done", "x"), 100);
    assert!(headers.contains(&("apns-priority", "5".into())));
    assert!(headers.contains(&("apns-expiration", "3700".into())));
    assert!(headers.contains(&("apns-push-type", "alert".into())));
    assert!(headers.contains(&("apns-topic", "dev.beagletools.beagle".into())));
    let urgent = super::headers(&pending("blocked", "x"), 100);
    assert!(urgent.contains(&("apns-priority", "10".into())));

    let big = payload(&pending("blocked", &"é".repeat(5000)));
    assert!(big.len() <= MAX_PAYLOAD, "{}", big.len());
    let big: Value = serde_json::from_slice(&big).unwrap();
    assert!(big["aps"]["alert"]["body"].as_str().unwrap().ends_with('…'));
}

#[test]
fn every_apns_answer_has_an_action() {
    assert_eq!(decide(Some(200), ""), Action::Done);
    assert_eq!(
        decide(Some(400), r#"{"reason":"BadDeviceToken"}"#),
        Action::OtherHost
    );
    assert_eq!(
        decide(Some(410), r#"{"reason":"Unregistered"}"#),
        Action::Forget
    );
    assert_eq!(
        decide(Some(403), r#"{"reason":"ExpiredProviderToken"}"#),
        Action::NewToken
    );
    assert!(matches!(
        decide(Some(403), r#"{"reason":"InvalidProviderToken"}"#),
        Action::Drop(_)
    ));
    assert!(matches!(
        decide(Some(400), r#"{"reason":"BadTopic"}"#),
        Action::Drop(_)
    ));
    for retry in [None, Some(429), Some(500), Some(503)] {
        assert_eq!(decide(retry, ""), Action::Retry, "{retry:?}");
    }
}

#[test]
fn a_test_push_reaches_a_fake_apns_correctly_signed() {
    let tmp = tempfile::tempdir().unwrap();
    let store = store_with(tmp.path(), Environment::Development);
    let fake = fake_apns(vec![]);
    let s = signer();
    let public = s.public_key().to_vec();
    let sender: SharedSender = Arc::new(Mutex::new(Box::new(
        ApnsSender::new(s, Arc::clone(&store))
            .unwrap()
            .with_host(&fake.url),
    )));
    let device = store.load().unwrap().devices[PHONE].clone();
    let outcome = send_test(&sender, PHONE, &device);
    assert!(outcome.delivered, "{}", outcome.message);

    let seen = fake.seen.lock().unwrap().clone();
    assert_eq!(seen.len(), 1);
    let req = &seen[0];
    assert_eq!(req.path, format!("/3/device/{}", "ab".repeat(32)));
    let header = |name: &str| req.headers.get(name).unwrap().to_str().unwrap().to_owned();
    let jwt = header("authorization")
        .strip_prefix("bearer ")
        .unwrap()
        .to_owned();
    verify(&jwt, &public);
    assert_eq!(header("apns-topic"), "dev.beagletools.beagle");
    assert_eq!(header("apns-push-type"), "alert");
    assert_eq!(header("apns-priority"), "10");
    assert_eq!(header("apns-id").len(), 36);
    let body: Value = serde_json::from_slice(&req.body).unwrap();
    assert_eq!(body["aps"]["alert"]["title"], "Beagle · test push");
    assert_eq!(body["aps"]["alert"]["subtitle"], "To iPhone");
    assert_eq!(body["aps"]["sound"], "default");
}

#[test]
fn a_token_from_the_other_environment_is_retried_there_and_remembered() {
    let tmp = tempfile::tempdir().unwrap();
    let store = store_with(tmp.path(), Environment::Development);
    let fake = fake_apns(vec![(400, "BadDeviceToken")]);
    let mut sender = ApnsSender::new(signer(), Arc::clone(&store))
        .unwrap()
        .with_host(&fake.url);
    assert!(sender.send(&pending("blocked", "x")).delivered);
    assert_eq!(fake.seen.lock().unwrap().len(), 2);
    assert_eq!(
        store.load().unwrap().devices[PHONE].environment,
        Environment::Production
    );
}

#[test]
fn dead_phones_are_forgotten_or_wait_for_a_new_token() {
    let tmp = tempfile::tempdir().unwrap();
    let store = store_with(tmp.path(), Environment::Development);
    let fake = fake_apns(vec![(410, "Unregistered")]);
    let mut sender = ApnsSender::new(signer(), Arc::clone(&store))
        .unwrap()
        .with_host(&fake.url);
    assert!(!sender.send(&pending("blocked", "x")).delivered);
    assert!(store.load().unwrap().devices.is_empty());

    let store = store_with(&tmp.path().join("b"), Environment::Development);
    let fake = fake_apns(vec![(400, "BadDeviceToken"), (400, "BadDeviceToken")]);
    let mut sender = ApnsSender::new(signer(), Arc::clone(&store))
        .unwrap()
        .with_host(&fake.url);
    assert!(!sender.send(&pending("blocked", "x")).delivered);
    assert_eq!(store.load().unwrap().devices[PHONE].apns_token, None);
}

#[test]
fn expired_tokens_are_renewed_and_outages_retried_three_times() {
    let tmp = tempfile::tempdir().unwrap();
    let store = store_with(tmp.path(), Environment::Development);
    let fake = fake_apns(vec![(403, "ExpiredProviderToken")]);
    let mut sender = ApnsSender::new(signer(), Arc::clone(&store))
        .unwrap()
        .with_host(&fake.url);
    assert!(sender.send(&pending("blocked", "x")).delivered);
    let seen = fake.seen.lock().unwrap().clone();
    assert_eq!(seen.len(), 2);
    assert_ne!(
        seen[0].headers["authorization"],
        seen[1].headers["authorization"]
    );

    let fake = fake_apns(vec![(503, "ServiceUnavailable"); 4]);
    let mut sender = ApnsSender::new(signer(), Arc::clone(&store))
        .unwrap()
        .with_host(&fake.url);
    assert!(!sender.send(&pending("blocked", "x")).delivered);
    assert_eq!(
        fake.seen.lock().unwrap().len(),
        4,
        "one try plus three retries"
    );
}

#[test]
fn the_key_must_be_private_and_the_ids_apples() {
    let tmp = tempfile::tempdir().unwrap();
    let key = tmp.path().join("AuthKey_KEY1234567.p8");
    std::fs::write(&key, p8()).unwrap();
    let push = PushConfig {
        team_id: "ABC123DEFG".into(),
        key_id: "KEY1234567".into(),
        key: key.clone(),
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(cmd::signer(&push).unwrap_err().contains("chmod 600"));
        std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    assert!(cmd::signer(&push).is_ok());
    let bad = PushConfig {
        team_id: "abc".into(),
        ..push
    };
    assert!(cmd::signer(&bad).unwrap_err().contains("team_id"));
}
