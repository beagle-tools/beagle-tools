//! The listener: accept, hand each connection to a short-lived thread,
//! answer one request, close.
//!
//! Connections are bounded ([`Limits::max`] at once; extras are dropped)
//! and each one must finish within [`Limits::deadline`], so a slow, idle or
//! half-open client can never pile up threads or wedge the server. Every
//! connection closed without an answer is logged with its reason
//! (rate-limited, [`Drops`]).

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::time::{Duration, Instant};

use rustls::{ServerConfig, ServerConnection, StreamOwned};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::conn::{Connections, Deadline, Drops, Limits};
use crate::http::{self, Method, ReadError, Request, Response};
use crate::router::{self, Ctx};

/// What every connection thread shares besides its socket.
struct Shared {
    ctx: Arc<Ctx>,
    tls: Option<Arc<ServerConfig>>,
    limits: Limits,
    conns: Connections,
    drops: Drops,
}

impl Shared {
    /// Logs a connection closed without an answer (rate-limited).
    fn dropped(&self, peer: &str, reason: &str) {
        if let Some(line) = self.drops.record(Instant::now(), peer, reason, &self.conns) {
            eprintln!("{} {line}", now());
        }
    }
}

/// Accepts connections forever. `tls` is `None` only for loopback testing
/// (`--no-tls`); the caller enforces that. `conns` counts the open
/// connections and caps them (give the same one to `/v1/health`).
pub fn serve(
    listener: &TcpListener,
    ctx: &Arc<Ctx>,
    tls: Option<&Arc<ServerConfig>>,
    conns: &Connections,
    limits: Limits,
) {
    let shared = Arc::new(Shared {
        ctx: Arc::clone(ctx),
        tls: tls.map(Arc::clone),
        limits,
        conns: conns.clone(),
        drops: Drops::default(),
    });
    for stream in listener.incoming() {
        let stream = match stream {
            Ok(stream) => stream,
            Err(e) => {
                eprintln!("{} accept failed: {e}", now());
                continue;
            }
        };
        let peer = peer_of(&stream);
        let Some(slot) = shared.conns.acquire() else {
            // Dropping the stream closes it: too busy to answer politely.
            shared.dropped(&peer, "busy, every connection slot is taken");
            continue;
        };
        let worker = Arc::clone(&shared);
        let worker_peer = peer.clone();
        let spawned = std::thread::Builder::new()
            .name("serve-conn".into())
            .spawn(move || {
                let _slot = slot;
                connection(stream, &worker_peer, &worker);
            });
        if let Err(e) = spawned {
            shared.dropped(&peer, &format!("could not start a thread: {e}"));
        }
    }
}

/// The client's IP, for logs.
fn peer_of(stream: &TcpStream) -> String {
    stream
        .peer_addr()
        .map_or_else(|_| "?".to_owned(), |a: SocketAddr| a.ip().to_string())
}

fn connection(stream: TcpStream, peer: &str, shared: &Shared) {
    let mut stream = Deadline::new(stream, shared.limits);
    let Some(config) = shared.tls.clone() else {
        match exchange(&mut stream, &shared.ctx, peer) {
            Some(line) => eprintln!("{line}"),
            None => shared.dropped(peer, closed_reason(&stream, false)),
        }
        return;
    };
    let conn = match ServerConnection::new(config) {
        Ok(conn) => conn,
        Err(e) => {
            shared.dropped(peer, &format!("TLS setup failed: {e}"));
            return;
        }
    };
    let mut tls_stream = StreamOwned::new(conn, stream);
    match exchange(&mut tls_stream, &shared.ctx, peer) {
        Some(line) => eprintln!("{line}"),
        None => shared.dropped(
            peer,
            closed_reason(&tls_stream.sock, tls_stream.conn.is_handshaking()),
        ),
    }
    tls_stream.conn.send_close_notify();
    // The response is already written; a failed close_notify only means the
    // client hung up first.
    let _ = tls_stream.flush();
}

/// Why a connection ended before a whole request arrived.
fn closed_reason<S>(stream: &Deadline<S>, handshaking: bool) -> &'static str {
    match (stream.expired(), handshaking) {
        (true, true) => "TLS handshake did not finish in time",
        (true, false) => "no complete request in time",
        (false, true) => "client left during the TLS handshake",
        (false, false) => "client left before sending a request",
    }
}

/// Reads one request from `stream`, answers it, and returns the log line
/// (`None` when the client sent nothing to answer, e.g. a failed TLS
/// handshake or an idle connection).
pub fn exchange<S: Read + Write>(stream: &mut S, ctx: &Ctx, peer: &str) -> Option<String> {
    let started = Instant::now();
    let (request, response) = match http::read_request(stream) {
        Ok(request) => {
            let response = router::handle(ctx, &request);
            (Some(request), response)
        }
        Err(ReadError::Closed) => return None,
        Err(ReadError::TooLarge) => (
            None,
            Response::error(413, "too_large", "request head or body over 16 KiB"),
        ),
        Err(ReadError::Malformed(why)) => (None, Response::error(400, "bad_request", why)),
    };
    let head_only = request.as_ref().is_some_and(|r| r.method == Method::Head);
    let written = response.write_to(stream, head_only);
    let line = log_line(
        &now(),
        peer,
        request.as_ref(),
        response.status,
        started.elapsed(),
    );
    Some(match written {
        Ok(()) => line,
        Err(e) => format!("{line} (write failed: {e})"),
    })
}

/// One request's log line: time, client, method, path (never the query or
/// headers, which could carry secrets), status, milliseconds.
#[must_use]
pub fn log_line(
    time: &str,
    peer: &str,
    request: Option<&Request>,
    status: u16,
    elapsed: Duration,
) -> String {
    let (method, path) = request.map_or(("-", "-"), |r| (r.method.as_str(), r.path.as_str()));
    format!(
        "{time} {peer} {method} {path} {status} {}ms",
        elapsed.as_millis()
    )
}

/// The current time, RFC 3339 in UTC (a log stamp; never fails visibly).
fn now() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "-".to_owned())
}

#[cfg(test)]
#[path = "tests/server.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/loopback.rs"]
mod loopback;
