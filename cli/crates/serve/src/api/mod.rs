//! The endpoints, one module per tool. Each reads its store through the
//! tool's own crate and returns JSON shaped as `docs/serve-api.md` says.

use serde_json::{json, Value};

use crate::http::Response;

/// The tools the API serves, for `/v1/version`.
pub const TOOLS: [&str; 5] = ["scout", "rca", "recap", "standup", "tandem"];

/// The API's version: bumped only on a breaking change to the documented
/// contract.
pub const API_VERSION: u32 = 1;

/// An endpoint's failure, mapped to the documented error body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiError {
    /// HTTP status.
    pub status: u16,
    /// The documented `code`.
    pub code: &'static str,
    /// What went wrong, for a person.
    pub message: String,
}

impl ApiError {
    /// `400 bad_request`.
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: 400,
            code: "bad_request",
            message: message.into(),
        }
    }

    /// `404 not_found`.
    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: 404,
            code: "not_found",
            message: message.into(),
        }
    }

    /// `500 internal`: a store could not be read.
    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            status: 500,
            code: "internal",
            message: message.into(),
        }
    }
}

impl From<ApiError> for Response {
    fn from(e: ApiError) -> Self {
        Response::error(e.status, e.code, e.message)
    }
}

/// What an endpoint returns.
pub type ApiResult = Result<Value, ApiError>;

/// `GET /v1/health`.
#[must_use]
pub fn health() -> Value {
    json!({ "ok": true })
}

/// `GET /v1/version`.
#[must_use]
pub fn version() -> Value {
    json!({
        "beagle": env!("CARGO_PKG_VERSION"),
        "api": API_VERSION,
        "tools": TOOLS,
    })
}
