//! Request → response: method check, authentication, then the endpoint.
//!
//! Pure apart from the stores the endpoints read, so it is tested without
//! sockets.

use crate::api;
use crate::auth::Token;
use crate::http::{Method, Request, Response};
use crate::stores::Stores;

/// Everything a request needs: the token it must carry, and the stores.
#[derive(Debug)]
pub struct Ctx {
    token: Token,
    stores: Stores,
}

impl Ctx {
    /// A context accepting `token`, serving `stores`.
    #[must_use]
    pub fn new(token: Token, stores: Stores) -> Self {
        Self { token, stores }
    }
}

/// Answers one request.
///
/// Order matters: the method is checked first, then `/v1/health` is
/// answered without a token, then everything else needs the token, so an
/// unauthenticated caller learns nothing about which paths exist.
#[must_use]
pub fn handle(ctx: &Ctx, req: &Request) -> Response {
    if let Method::Other(method) = &req.method {
        return Response::error(
            405,
            "method_not_allowed",
            format!("{method} is not supported: this API is read-only (GET, HEAD)"),
        );
    }
    let segments: Vec<&str> = req
        .path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    if segments == ["v1", "health"] {
        return Response::ok(api::health());
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
        ["v1", "scout", rest @ ..] => api::root("scout", &ctx.stores.scout)
            .and_then(|root| api::scout::route(root, rest, req)),
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
