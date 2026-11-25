//! `/v1/devices/…`: phones register for push and keep their notification
//! settings here (`docs/serve-push.md`). The only routes that write.

use serde_json::{json, Value};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use super::ApiError;
use crate::http::{Method, Request, Response};
use crate::push::devices::{device_id, Device, DeviceStore, Registration, MAX_DEVICES};
use crate::push::settings::{self, Settings};
use crate::push::{send_test, SharedSender};

/// Routes `rest` (the path after `/v1/`, starting at `devices`).
///
/// # Errors
/// [`ApiError`] for bad ids and bodies, unknown devices and store failures.
pub fn route(
    store: &DeviceStore,
    push: Option<&SharedSender>,
    rest: &[&str],
    req: &Request,
) -> Result<Response, ApiError> {
    let (id, tail) = match rest {
        ["devices", id, tail @ ..] => (*id, tail),
        _ => return Err(ApiError::not_found(format!("no endpoint {}", req.path))),
    };
    let id = device_id(id).ok_or_else(|| ApiError::bad_request("the device id must be a UUID"))?;
    match (tail, &req.method) {
        ([], Method::Put) => register(store, &id, req),
        ([], Method::Delete) => {
            store
                .update(|d| d.devices.remove(&id))
                .map_err(|e| internal(&e))?;
            Ok(Response::no_content())
        }
        (["notifications"], Method::Get | Method::Head) => {
            let devices = store.load().map_err(|e| internal(&e))?;
            let device = devices.devices.get(&id).ok_or_else(|| unknown(&id))?;
            Ok(Response::ok(document(&settings::complete(
                &device.settings,
            ))))
        }
        (["notifications"], Method::Put) => update_settings(store, &id, req),
        (["test"], Method::Post) => {
            let devices = store.load().map_err(|e| internal(&e))?;
            let device = devices.devices.get(&id).ok_or_else(|| unknown(&id))?;
            let push = push
                .filter(|s| s.lock().is_ok_and(|s| s.configured()))
                .ok_or_else(|| ApiError {
                    status: 503,
                    code: "push_not_configured",
                    message: "push isn't set up on the Mac yet: run `beagle serve push setup`"
                        .into(),
                })?;
            let outcome = send_test(push, &id, device);
            if outcome.delivered {
                Ok(Response::ok(
                    json!({ "delivered": true, "message": outcome.message }),
                ))
            } else {
                Err(ApiError {
                    status: 502,
                    code: "push_failed",
                    message: outcome.message,
                })
            }
        }
        ([], _) => Ok(not_allowed("PUT, DELETE")),
        (["notifications"], _) => Ok(not_allowed("GET, HEAD, PUT")),
        (["test"], _) => Ok(not_allowed("POST")),
        _ => Err(ApiError::not_found(format!("no endpoint {}", req.path))),
    }
}

/// `PUT /v1/devices/{id}`: register or refresh a phone.
fn register(store: &DeviceStore, id: &str, req: &Request) -> Result<Response, ApiError> {
    let reg: Registration = parse_json(req)?;
    reg.validate().map_err(ApiError::bad_request)?;
    let now = now();
    let outcome = store
        .update(|d| {
            if let Some(device) = d.devices.get_mut(id) {
                device.apns_token = Some(reg.apns_token.to_ascii_lowercase());
                device.environment = reg.environment;
                device.topic.clone_from(&reg.topic);
                reg.name.trim().clone_into(&mut device.name);
                device.last_seen.clone_from(&now);
                return Ok((false, settings::complete(&device.settings)));
            }
            if d.devices.len() >= MAX_DEVICES {
                return Err(ApiError {
                    status: 409,
                    code: "too_many_devices",
                    message: format!("this Mac already has {MAX_DEVICES} phones registered"),
                });
            }
            let fresh = settings::defaults();
            d.devices.insert(
                id.to_owned(),
                Device {
                    apns_token: Some(reg.apns_token.to_ascii_lowercase()),
                    environment: reg.environment,
                    topic: reg.topic.clone(),
                    name: reg.name.trim().to_owned(),
                    registered: now.clone(),
                    last_seen: now.clone(),
                    settings: fresh.clone(),
                },
            );
            Ok((true, fresh))
        })
        .map_err(|e| internal(&e))?;
    let (created, settings) = outcome?;
    let body = document(&settings);
    Ok(if created {
        Response::created(body)
    } else {
        Response::ok(body)
    })
}

/// `PUT /v1/devices/{id}/notifications`: replace a phone's settings.
fn update_settings(store: &DeviceStore, id: &str, req: &Request) -> Result<Response, ApiError> {
    let raw: Value = parse_json(req)?;
    let wanted = settings::parse(&raw).map_err(ApiError::bad_request)?;
    let saved = store
        .update(|d| {
            d.devices.get_mut(id).map(|device| {
                device.settings = wanted.clone();
            })
        })
        .map_err(|e| internal(&e))?;
    saved.ok_or_else(|| unknown(id))?;
    Ok(Response::ok(document(&wanted)))
}

/// The settings document: the phone's settings plus the catalog.
fn document(settings: &Settings) -> Value {
    json!({ "settings": settings, "catalog": settings::catalog_json() })
}

/// The body as `T`: `415` when it isn't declared JSON, `400` when it
/// doesn't parse (unknown fields included).
fn parse_json<T: serde::de::DeserializeOwned>(req: &Request) -> Result<T, ApiError> {
    if !req.is_json() {
        return Err(ApiError {
            status: 415,
            code: "unsupported_media_type",
            message: "send the body as Content-Type: application/json".into(),
        });
    }
    serde_json::from_slice(&req.body)
        .map_err(|e| ApiError::bad_request(format!("bad JSON body: {e}")))
}

fn unknown(id: &str) -> ApiError {
    ApiError::not_found(format!(
        "no device {id}: register it with PUT /v1/devices/{id}"
    ))
}

/// `405`, with the methods this endpoint does take in `Allow`.
fn not_allowed(allow: &str) -> Response {
    let mut response = Response::error(
        405,
        "method_not_allowed",
        format!("this endpoint takes {allow}"),
    );
    response.headers.retain(|(name, _)| *name != "Allow");
    response.headers.push(("Allow", allow.to_owned()));
    response
}

fn internal(e: &crate::Error) -> ApiError {
    ApiError::internal(format!("device store: {e}"))
}

fn now() -> String {
    OffsetDateTime::now_utc()
        .replace_nanosecond(0)
        .ok()
        .and_then(|t| t.format(&Rfc3339).ok())
        .unwrap_or_default()
}

#[cfg(test)]
#[path = "../tests/devices.rs"]
mod tests;
