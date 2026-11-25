//! Sending through Apple's Push Notification service: the request
//! (`docs/serve-push.md`, "Sending through APNs"), what each answer means,
//! and [`ApnsSender`], which also cleans up dead phones.

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

use super::devices::{DeviceStore, Environment};
use super::jwt::Signer;
use super::{settings, Outcome, Pending, Sender};

/// APNs' production host.
pub const PRODUCTION: &str = "https://api.push.apple.com";
/// APNs' sandbox host (Xcode installs).
pub const SANDBOX: &str = "https://api.sandbox.push.apple.com";
/// APNs refuses payloads over 4 KiB.
pub const MAX_PAYLOAD: usize = 4096;
/// Waits before each retry of a `429`, `5xx` or connection failure.
pub const RETRY_AFTER: [Duration; 3] = [
    Duration::from_secs(1),
    Duration::from_secs(4),
    Duration::from_secs(16),
];

/// The JSON payload for `p`: the banner's text, threaded per project, a
/// sound only where the tool's catalog says, and the app link.
#[must_use]
pub fn payload(p: &Pending) -> Vec<u8> {
    let r = &p.record;
    let sound = settings::kind(&r.tool, &r.kind).is_some_and(|k| k.sound) || r.kind == "test";
    let mut body: String = r.body.clone();
    loop {
        let mut aps = json!({
            "alert": { "title": r.title, "subtitle": r.subtitle, "body": body },
            "thread-id": format!("{}-{}", r.tool, r.project),
        });
        if sound {
            aps["sound"] = json!("default");
        }
        let bytes = json!({
            "aps": aps,
            "beagle": { "tool": r.tool, "kind": r.kind, "link": r.link },
        })
        .to_string()
        .into_bytes();
        if bytes.len() <= MAX_PAYLOAD || body.is_empty() {
            return bytes;
        }
        // Over the limit: cut the body to fit (on a char boundary, leaving
        // room for the ellipsis) and try again.
        let budget = body
            .len()
            .saturating_sub(bytes.len() - MAX_PAYLOAD + '…'.len_utf8());
        let mut cut = String::new();
        for c in body.chars() {
            if cut.len() + c.len_utf8() > budget {
                break;
            }
            cut.push(c);
        }
        body = if cut.is_empty() { cut } else { cut + "…" };
    }
}

/// The request headers for `p` (without `authorization`), at unix time `now`.
#[must_use]
pub fn headers(p: &Pending, now: u64) -> Vec<(&'static str, String)> {
    let urgent = p.record.kind == "blocked" || p.record.kind == "test";
    let lifetime = if urgent { 24 * 3600 } else { 3600 };
    vec![
        ("apns-topic", p.topic.clone()),
        ("apns-push-type", "alert".to_owned()),
        ("apns-priority", if urgent { "10" } else { "5" }.to_owned()),
        ("apns-expiration", (now + lifetime).to_string()),
        ("apns-id", p.record.id.clone()),
    ]
}

/// What to do after an APNs answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Delivered.
    Done,
    /// Try the other environment's host once (`BadDeviceToken`).
    OtherHost,
    /// The app is gone from the phone (`410`): forget the phone.
    Forget,
    /// A new JWT, then retry once (`ExpiredProviderToken`).
    NewToken,
    /// Wait and retry (`429`, `5xx`, no answer).
    Retry,
    /// Give up on this one, with why (our mistake, or a config problem).
    Drop(String),
}

/// The action for status `status` with body `body` (`None`: no answer).
#[must_use]
pub fn decide(status: Option<u16>, body: &str) -> Action {
    let reason = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| v["reason"].as_str().map(str::to_owned))
        .unwrap_or_default();
    match status {
        Some(200) => Action::Done,
        Some(400) if reason == "BadDeviceToken" => Action::OtherHost,
        Some(410) => Action::Forget,
        Some(403) if reason == "ExpiredProviderToken" => Action::NewToken,
        Some(403) => Action::Drop(format!(
            "APNs refused the key ({reason}): check team_id, key_id and the .p8 in serve.toml (`beagle serve push setup`)"
        )),
        None | Some(429 | 500..=599) => Action::Retry,
        Some(code) => Action::Drop(format!("APNs answered {code} {reason}")),
    }
}

/// The host for `environment`.
#[must_use]
pub fn host(environment: Environment) -> &'static str {
    match environment {
        Environment::Development => SANDBOX,
        Environment::Production => PRODUCTION,
    }
}

/// Sends through APNs over HTTP/2, signing with the `.p8` key.
#[derive(Debug)]
pub struct ApnsSender {
    signer: Signer,
    client: reqwest::blocking::Client,
    store: Arc<DeviceStore>,
    /// Replaces both hosts (tests point it at a fake server).
    host_override: Option<String>,
    /// The waits between retries ([`RETRY_AFTER`]; tests shorten them).
    retry_after: Vec<Duration>,
}

impl ApnsSender {
    /// A sender signing with `signer` that updates `store` when a phone's
    /// token turns out dead or in the other environment.
    ///
    /// # Errors
    /// The HTTP client could not be built.
    pub fn new(signer: Signer, store: Arc<DeviceStore>) -> Result<Self, String> {
        let client = reqwest::blocking::Client::builder()
            .http2_prior_knowledge()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|e| format!("could not set up the HTTP/2 client: {e}"))?;
        Ok(Self {
            signer,
            client,
            store,
            host_override: None,
            retry_after: RETRY_AFTER.to_vec(),
        })
    }

    /// Sends to `host` instead of Apple's (tests), with no waits between
    /// retries.
    #[must_use]
    pub fn with_host(mut self, host: &str) -> Self {
        self.host_override = Some(host.to_owned());
        self.retry_after = vec![Duration::ZERO; RETRY_AFTER.len()];
        self
    }

    /// One POST; `None` status when there was no answer.
    fn post(&mut self, p: &Pending, environment: Environment) -> (Option<u16>, String) {
        let now = unix_now();
        let token = match self.signer.token(now) {
            Ok(token) => token,
            Err(e) => return (Some(0), json!({ "reason": e }).to_string()),
        };
        let base = self
            .host_override
            .clone()
            .unwrap_or_else(|| host(environment).to_owned());
        let mut request = self
            .client
            .post(format!("{base}/3/device/{}", p.apns_token))
            .header("authorization", format!("bearer {token}"))
            .body(payload(p));
        for (name, value) in headers(p, now) {
            request = request.header(name, value);
        }
        match request.send() {
            Ok(response) => {
                let status = response.status().as_u16();
                (Some(status), response.text().unwrap_or_default())
            }
            Err(e) => (None, json!({ "reason": e.to_string() }).to_string()),
        }
    }
}

impl Sender for ApnsSender {
    fn send(&mut self, p: &Pending) -> Outcome {
        let mut environment = p.environment;
        let mut switched = false;
        let mut renewed = false;
        let mut retries = self.retry_after.clone().into_iter();
        loop {
            let (status, body) = self.post(p, environment);
            match decide(status, &body) {
                Action::Done => {
                    if switched {
                        let _ = self.store.update(|d| {
                            if let Some(device) = d.devices.get_mut(&p.device) {
                                device.environment = environment;
                            }
                        });
                    }
                    return Outcome::sent(format!("delivered ({})", p.record.id));
                }
                Action::OtherHost if !switched => {
                    switched = true;
                    environment = match environment {
                        Environment::Development => Environment::Production,
                        Environment::Production => Environment::Development,
                    };
                }
                Action::OtherHost => {
                    // Wrong in both environments: the app re-registers on its next launch.
                    let _ = self.store.update(|d| {
                        if let Some(device) = d.devices.get_mut(&p.device) {
                            device.apns_token = None;
                        }
                    });
                    return Outcome::failed("APNs rejected the phone's token in both environments; it will re-register on next launch".into());
                }
                Action::Forget => {
                    let _ = self.store.update(|d| d.devices.remove(&p.device));
                    return Outcome::failed(
                        "the app is no longer on that phone (410): forgot it".into(),
                    );
                }
                Action::NewToken if !renewed => {
                    renewed = true;
                    self.signer.expire();
                }
                Action::NewToken => {
                    return Outcome::failed("APNs keeps rejecting new tokens".into())
                }
                Action::Retry => match retries.next() {
                    Some(wait) => std::thread::sleep(wait),
                    None => {
                        return Outcome::failed(format!("APNs didn't answer after retries: {body}"))
                    }
                },
                Action::Drop(why) => return Outcome::failed(why),
            }
        }
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[cfg(test)]
#[path = "../tests/apns.rs"]
mod tests;
