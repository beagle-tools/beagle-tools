//! Just enough HTTP/1.1 for a small JSON API: read a request (head and any
//! body), write a complete response, close.
//!
//! Requests are `GET`/`HEAD` (the read API) or `PUT`/`DELETE`/`POST` with a
//! small JSON body (the device API, `docs/serve-push.md`). Bodies need
//! `Content-Length` (no chunking) and are capped at [`MAX_BODY_BYTES`];
//! heads at [`MAX_HEAD_BYTES`]. Every response carries `Connection: close`,
//! so there is no keep-alive or pipelining to get wrong.

use std::io::{self, Read, Write};

use serde_json::{json, Value};

/// The largest request head (request line + headers) accepted.
pub const MAX_HEAD_BYTES: usize = 16 * 1024;

/// The largest request body accepted.
pub const MAX_BODY_BYTES: usize = 16 * 1024;

/// The request method, as far as this server cares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Method {
    /// `GET`.
    Get,
    /// `HEAD`: a `GET` without the body.
    Head,
    /// `PUT` (device API only).
    Put,
    /// `DELETE` (device API only).
    Delete,
    /// `POST` (device API only).
    Post,
    /// Anything else (answered `405`).
    Other(String),
}

impl Method {
    /// The method as sent.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Get => "GET",
            Self::Head => "HEAD",
            Self::Put => "PUT",
            Self::Delete => "DELETE",
            Self::Post => "POST",
            Self::Other(m) => m,
        }
    }
}

/// A parsed request.
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
    /// The `Content-Type` header's value, if any.
    pub content_type: Option<String>,
    /// The body (empty without one).
    pub body: Vec<u8>,
}

impl Request {
    /// A `GET` of `target` (path plus optional `?query`), for tests and
    /// callers that build requests directly.
    #[must_use]
    pub fn get(target: &str, authorization: Option<&str>) -> Self {
        Self::new(Method::Get, target, authorization, None)
    }

    /// A request with an optional JSON body, for tests and callers that
    /// build requests directly.
    #[must_use]
    pub fn new(
        method: Method,
        target: &str,
        authorization: Option<&str>,
        json: Option<&Value>,
    ) -> Self {
        let (path, query) = target.split_once('?').unwrap_or((target, ""));
        Self {
            method,
            path: path.to_owned(),
            query: query.to_owned(),
            authorization: authorization.map(str::to_owned),
            content_type: json.map(|_| "application/json".to_owned()),
            body: json.map(|j| j.to_string().into_bytes()).unwrap_or_default(),
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

    /// Whether the body is declared as JSON (`application/json`, any
    /// parameters).
    #[must_use]
    pub fn is_json(&self) -> bool {
        self.content_type.as_deref().is_some_and(|t| {
            t.split(';')
                .next()
                .is_some_and(|t| t.trim().eq_ignore_ascii_case("application/json"))
        })
    }
}

/// Why no request could be read.
#[derive(Debug)]
pub enum ReadError {
    /// The peer closed (or timed out) before sending a full request.
    /// Nothing is answered.
    Closed,
    /// The head is larger than [`MAX_HEAD_BYTES`], or the body than
    /// [`MAX_BODY_BYTES`] (`413`).
    TooLarge,
    /// Not an HTTP/1.x request (`400`).
    Malformed(&'static str),
}

/// Reads one request (head, then a `Content-Length` body) from `stream`.
///
/// # Errors
/// [`ReadError`] as documented on its variants.
pub fn read_request<R: Read>(stream: &mut R) -> Result<Request, ReadError> {
    let mut buf = Vec::with_capacity(1024);
    let mut chunk = [0u8; 1024];
    let end = loop {
        if let Some(end) = find_head_end(&buf) {
            break end;
        }
        if buf.len() > MAX_HEAD_BYTES {
            return Err(ReadError::TooLarge);
        }
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => return Err(ReadError::Closed),
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
    };
    if end > MAX_HEAD_BYTES {
        return Err(ReadError::TooLarge);
    }
    let text = std::str::from_utf8(&buf[..end]).map_err(|_| ReadError::Malformed("not UTF-8"))?;
    let (mut request, length) = parse_head(text)?;
    if length > MAX_BODY_BYTES {
        return Err(ReadError::TooLarge);
    }
    let mut body = buf[end..].to_vec();
    while body.len() < length {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => return Err(ReadError::Closed),
            Ok(n) => body.extend_from_slice(&chunk[..n]),
        }
    }
    body.truncate(length);
    request.body = body;
    Ok(request)
}

/// The length of the head (through the blank line), once it is complete.
fn find_head_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n").map(|i| i + 4)
}

/// The request (without its body) and the body's declared length.
fn parse_head(text: &str) -> Result<(Request, usize), ReadError> {
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
        "PUT" => Method::Put,
        "DELETE" => Method::Delete,
        "POST" => Method::Post,
        other => Method::Other(other.to_owned()),
    };
    let mut request = Request::new(method, target, None, None);
    let mut length = 0;
    for line in lines.filter(|l| !l.is_empty()) {
        let (name, value) = line
            .split_once(':')
            .ok_or(ReadError::Malformed("bad header line"))?;
        let (name, value) = (name.trim(), value.trim());
        if name.eq_ignore_ascii_case("authorization") {
            request.authorization = Some(value.to_owned());
        } else if name.eq_ignore_ascii_case("content-type") {
            request.content_type = Some(value.to_owned());
        } else if name.eq_ignore_ascii_case("content-length") {
            length = value
                .parse()
                .map_err(|_| ReadError::Malformed("bad Content-Length"))?;
        } else if name.eq_ignore_ascii_case("transfer-encoding") {
            return Err(ReadError::Malformed(
                "chunked bodies are not supported: send Content-Length",
            ));
        }
    }
    Ok((request, length))
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

    /// `201` with `body`.
    #[must_use]
    pub fn created(body: Value) -> Self {
        Self {
            status: 201,
            body,
            headers: Vec::new(),
        }
    }

    /// `204`: done, nothing to say.
    #[must_use]
    pub fn no_content() -> Self {
        Self {
            status: 204,
            body: Value::Null,
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
        // A 204 has no body, so no Content-Type or Content-Length either.
        let (body, mut head) = if self.status == 204 {
            (
                Vec::new(),
                "HTTP/1.1 204 No Content\r\nCache-Control: no-store\r\nConnection: close\r\n"
                    .to_owned(),
            )
        } else {
            let body = serde_json::to_vec(&self.body).map_err(io::Error::other)?;
            let head = format!(
                "HTTP/1.1 {} {}\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n",
                self.status,
                reason(self.status),
                body.len()
            );
            (body, head)
        };
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
        201 => "Created",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        413 => "Content Too Large",
        415 => "Unsupported Media Type",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "Unknown",
    }
}

#[cfg(test)]
#[path = "tests/http.rs"]
mod tests;
