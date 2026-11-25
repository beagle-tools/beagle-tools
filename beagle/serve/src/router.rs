//! Request → response: method check, authentication, then the endpoint.
//!
//! Pure apart from the stores the endpoints read (and `devices.json`,
//! which the device API writes), so it is tested without sockets.

use std::sync::Arc;

use crate::api;
use crate::auth::Token;
use crate::conn::Connections;
use crate::http::{Method, Request, Response};
use crate::push::devices::DeviceStore;
use crate::push::SharedSender;
use crate::stores::Stores;

/// Everything a request needs: the token it must carry, the stores, and
/// the registered phones.
#[derive(Debug)]
pub struct Ctx {
    token: Token,
    stores: Stores,
    devices: Option<Arc<DeviceStore>>,
    push: Option<SharedSender>,
    conns: Option<Connections>,
}

impl Ctx {
    /// A context accepting `token`, serving `stores`, with no device store
    /// (the device API answers `500` until [`Ctx::with_devices`]).
    #[must_use]
    pub fn new(token: Token, stores: Stores) -> Self {
        Self {
            token,
            stores,
            devices: None,
            push: None,
            conns: None,
        }
    }

    /// The same context, keeping phones in `devices`.
    #[must_use]
    pub fn with_devices(mut self, devices: Arc<DeviceStore>) -> Self {
        self.devices = Some(devices);
        self
    }

    /// The same context, sending test pushes through `push`.
    #[must_use]
    pub fn with_push(mut self, push: SharedSender) -> Self {
        self.push = Some(push);
        self
    }

    /// The same context, reporting the open connections in `/v1/health`.
    #[must_use]
    pub fn with_connections(mut self, conns: Connections) -> Self {
        self.conns = Some(conns);
        self
    }
}

/// Answers one request.
///
/// Order matters: the method is checked first, then `/v1/health` is
/// answered without a token, then everything else needs the token, so an
/// unauthenticated caller learns nothing about which paths exist. Only
/// `/v1/devices/…` takes methods that write.
#[must_use]
pub fn handle(ctx: &Ctx, req: &Request) -> Response {
    if let Method::Other(method) = &req.method {
        return Response::error(
            405,
            "method_not_allowed",
            format!("{method} is not supported (GET, HEAD; PUT, DELETE, POST on /v1/devices)"),
        );
    }
    let segments: Vec<&str> = req
        .path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    let writes = matches!(req.method, Method::Put | Method::Delete | Method::Post);
    let devices = segments.get(1) == Some(&"devices") && segments.first() == Some(&"v1");
    if writes && !devices {
        return Response::error(
            405,
            "method_not_allowed",
            format!(
                "{} is not supported here: this endpoint is read-only (GET, HEAD)",
                req.method.as_str()
            ),
        );
    }
    if segments == ["v1", "health"] {
        return Response::ok(api::health(ctx.conns.as_ref()));
    }
    if !ctx.token.authorizes(req.authorization.as_deref()) {
        return Response::error(
            401,
            "unauthorized",
            "missing or wrong bearer token: pair again with `beagle serve pair`",
        );
    }
    let result = match segments.as_slice() {
        ["v1", "version"] => Ok(api::version()),
        ["v1", rest @ ..] if devices => {
            let Some(store) = &ctx.devices else {
                return api::ApiError::internal("no device store").into();
            };
            return api::devices::route(store, ctx.push.as_ref(), rest, req)
                .unwrap_or_else(Into::into);
        }
        ["v1", "scout", rest @ ..] => api::root("scout", &ctx.stores.scout)
            .and_then(|root| api::scout::route(root, rest, req)),
        ["v1", "rca", rest @ ..] => {
            api::root("rca", &ctx.stores.rca).and_then(|root| api::rca::route(root, rest, req))
        }
        ["v1", "recap", rest @ ..] => api::root("recap", &ctx.stores.recap)
            .and_then(|root| api::recap::route(root, rest, req)),
        ["v1", "standup", rest @ ..] => api::root("standup", &ctx.stores.standup)
            .and_then(|root| api::standup::route(root, rest, req)),
        ["v1", "tandem", rest @ ..] => {
            api::root("tandem", &ctx.stores.tandem).and_then(|db| api::tandem::route(db, rest, req))
        }
        _ => Err(api::ApiError::not_found(format!(
            "no endpoint {}",
            req.path
        ))),
    };
    match result {
        Ok(body) => Response::ok(body),
        Err(e) => e.into(),
    }
}

#[cfg(test)]
#[path = "tests/router.rs"]
pub(crate) mod tests;
