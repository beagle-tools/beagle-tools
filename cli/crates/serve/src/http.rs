//! Just enough HTTP/1.1 for a read-only JSON API: read a request head,
//! write a complete response, close.
//!
//! Every request is a `GET` (or `HEAD`) with no body, and every response
//! carries `Content-Length` and `Connection: close`, so there is no
//! keep-alive, chunking or pipelining to get wrong. Request heads are
//! capped at [`MAX_HEAD_BYTES`].

use std::io::{self, Read, Write};

use serde_json::{json, Value};

/// The largest request head (request line + headers) accepted.
pub const MAX_HEAD_BYTES: usize = 16 * 1024;

/// The request method, as far as this server cares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Method {
    /// `GET`.
    Get,
    /// `HEAD`: a `GET` without the body.
    Head,
    /// Anything else (answered `405`).
    Other(String),
}

/// A parsed request head.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// The method.
    pub method: Method,
    /// The path, without the query (`/v1/rca`).
    pub path: String,
    /// The query string without the `?` (empty when there is none).
    pub query: String,
    /// The `Authorization` header's value, if any.
    pub authorization: Option<String>,
}

impl Request {
    /// A `GET` of `target` (path plus optional `?query`), for tests and
    /// callers that build requests directly.
    #[must_use]
    pub fn get(target: &str, authorization: Option<&str>) -> Self {
        let (path, query) = target.split_once('?').unwrap_or((target, ""));
        Self {
            method: Method::Get,
            path: path.to_owned(),
            query: query.to_owned(),
            authorization: authorization.map(str::to_owned),
        }
    }

    /// The value of query parameter `name` (first occurrence, not decoded:
    /// every parameter this API takes is a plain word or number).
    #[must_use]
    pub fn param(&self, name: &str) -> Option<&str> {
        self.query
            .split('&')
            .filter_map(|pair| pair.split_once('='))
            .find(|(k, _)| *k == name)
            .map(|(_, v)| v)
    }
}

/// Why no request could be read.
#[derive(Debug)]
pub enum ReadError {
    /// The peer closed (or timed out) before sending a full head. Nothing
    /// is answered.
    Closed,
    /// The head is larger than [`MAX_HEAD_BYTES`] (`413`).
    TooLarge,
    /// Not an HTTP/1.x request head (`400`).
    Malformed(&'static str),
}

/// Reads one request head from `stream`.
///
/// # Errors
/// [`ReadError`] as documented on its variants.
pub fn read_request<R: Read>(stream: &mut R) -> Result<Request, ReadError> {
    let mut head = Vec::with_capacity(1024);
    let mut chunk = [0u8; 1024];
    let end = loop {
        if let Some(end) = find_head_end(&head) {
            break end;
        }
        if head.len() > MAX_HEAD_BYTES {
            return Err(ReadError::TooLarge);
        }
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => return Err(ReadError::Closed),
            Ok(n) => head.extend_from_slice(&chunk[..n]),
        }
    };
    if end > MAX_HEAD_BYTES {
        return Err(ReadError::TooLarge);
    }
    let text = std::str::from_utf8(&head[..end]).map_err(|_| ReadError::Malformed("not UTF-8"))?;
    parse_head(text)
}

/// The length of the head (through the blank line), once it is complete.
fn find_head_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n").map(|i| i + 4)
}

fn parse_head(text: &str) -> Result<Request, ReadError> {
    let mut lines = text.split("\r\n");
    let request_line = lines.next().unwrap_or_default();
    let mut parts = request_line.split(' ');
    let (Some(method), Some(target), Some(version), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(ReadError::Malformed("bad request line"));
    };
    if !version.starts_with("HTTP/1.") {
        return Err(ReadError::Malformed("not HTTP/1.x"));
    }
    if !target.starts_with('/') {
        return Err(ReadError::Malformed("target is not a path"));
    }
    let method = match method {
        "GET" => Method::Get,
        "HEAD" => Method::Head,
        other => Method::Other(other.to_owned()),
    };
    let mut authorization = None;
    for line in lines.filter(|l| !l.is_empty()) {
        let (name, value) = line
            .split_once(':')
            .ok_or(ReadError::Malformed("bad header line"))?;
        if name.trim().eq_ignore_ascii_case("authorization") {
            authorization = Some(value.trim().to_owned());
        }
    }
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    Ok(Request {
        method,
        path: path.to_owned(),
        query: query.to_owned(),
        authorization,
    })
}

/// A complete response: status, JSON body, any extra headers.
#[derive(Debug, Clone, PartialEq)]
pub struct Response {
    /// The status code.
    pub status: u16,
    /// The JSON body.
    pub body: Value,
    /// Headers beyond the standard ones.
    pub headers: Vec<(&'static str, String)>,
}

impl Response {
    /// `200` with `body`.
    #[must_use]
    pub fn ok(body: Value) -> Self {
        Self {
            status: 200,
            body,
            headers: Vec::new(),
        }
    }

    /// An error response with the documented body:
    /// `{"error": {"code", "message"}}`.
    #[must_use]
    pub fn error(status: u16, code: &str, message: impl Into<String>) -> Self {
        let mut response = Self {
            status,
            body: json!({ "error": { "code": code, "message": message.into() } }),
            headers: Vec::new(),
        };
        if status == 401 {
            response
                .headers
                .push(("WWW-Authenticate", "Bearer".to_owned()));
        }
        if status == 405 {
            response.headers.push(("Allow", "GET, HEAD".to_owned()));
        }
        response
    }

    /// Writes the response; `head_only` (a `HEAD` request) leaves the body
    /// out but keeps its `Content-Length`.
    ///
    /// # Errors
    /// The stream's write error.
    pub fn write_to<W: Write>(&self, out: &mut W, head_only: bool) -> io::Result<()> {
        let body = serde_json::to_vec(&self.body).map_err(io::Error::other)?;
        let mut head = format!(
            "HTTP/1.1 {} {}\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n",
            self.status,
            reason(self.status),
            body.len()
        );
        for (name, value) in &self.headers {
            head.push_str(name);
            head.push_str(": ");
            head.push_str(value);
            head.push_str("\r\n");
        }
        head.push_str("\r\n");
        out.write_all(head.as_bytes())?;
        if !head_only {
            out.write_all(&body)?;
        }
        out.flush()
    }
}

/// The reason phrase for the statuses this server sends.
fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        405 => "Method Not Allowed",
        413 => "Request Header Fields Too Large",
        500 => "Internal Server Error",
        _ => "Unknown",
    }
}

#[cfg(test)]
#[path = "tests/http.rs"]
mod tests;
