//! The endpoints, one module per tool. Each reads its store through the
//! tool's own crate and returns JSON shaped as `docs/serve-api.md` says.

pub mod rca;
pub mod recap;
pub mod scout;
pub mod standup;
pub mod tandem;

use std::path::Path;
use std::time::SystemTime;

use serde_json::{json, Value};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::http::Response;
use crate::stores::Source;

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

/// The store root behind `source`, or a `500` saying why there is none.
///
/// # Errors
/// [`ApiError::internal`] naming the tool.
pub fn root<'a>(tool: &str, source: &'a Source) -> Result<&'a Path, ApiError> {
    source
        .as_deref()
        .map_err(|e| ApiError::internal(format!("{tool} store unavailable: {e}")))
}

/// A timestamp as RFC 3339 (UTC offset kept), `null` when absent.
#[must_use]
pub fn timestamp(t: Option<OffsetDateTime>) -> Value {
    t.and_then(|t| t.format(&Rfc3339).ok())
        .map_or(Value::Null, Value::String)
}

/// A file time as RFC 3339 in UTC, `null` when absent.
#[must_use]
pub fn file_time(t: Option<SystemTime>) -> Value {
    timestamp(t.and_then(|t| OffsetDateTime::from(t).replace_nanosecond(0).ok()))
}

/// `?lines=` for log tails: default 200, clamped to 1..=2000; anything
/// unparseable is a `400`.
///
/// # Errors
/// [`ApiError::bad_request`] for a non-number.
pub fn lines_param(raw: Option<&str>) -> Result<usize, ApiError> {
    match raw {
        None => Ok(DEFAULT_LINES),
        Some(raw) => raw
            .parse::<usize>()
            .map(|n| n.clamp(1, MAX_LINES))
            .map_err(|_| ApiError::bad_request(format!("lines must be a number, not {raw:?}"))),
    }
}

/// The last `n` lines of `text`, and whether any were left out.
#[must_use]
pub fn tail(text: &str, n: usize) -> (Vec<&str>, bool) {
    let all: Vec<&str> = text.lines().collect();
    let skip = all.len().saturating_sub(n);
    (all[skip..].to_vec(), skip > 0)
}

/// A diagram as the API serves it: `text` with ANSI escapes stripped (draw
/// it monospaced, unwrapped), `ansi` the raw file with its colour codes.
#[must_use]
pub fn diagram(name: &str, raw: &str) -> Value {
    json!({ "name": name, "text": beagle_rca::ansi::strip(raw), "ansi": raw })
}

/// Default log tail length.
pub const DEFAULT_LINES: usize = 200;
/// Longest log tail served.
pub const MAX_LINES: usize = 2000;

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
