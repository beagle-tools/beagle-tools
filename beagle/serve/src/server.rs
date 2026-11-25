//! The listener: accept, hand each connection to a short-lived thread,
//! answer one request, close.
//!
//! Connections are bounded ([`MAX_CONNECTIONS`] at once; extras are
//! dropped) and every socket has read and write timeouts, so a slow or
//! stuck client can never pile up threads or wedge the server.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use rustls::{ServerConfig, ServerConnection, StreamOwned};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::http::{self, Method, ReadError, Request, Response};
use crate::router::{self, Ctx};

/// Connections served at once; more are closed straight away.
pub const MAX_CONNECTIONS: usize = 32;

/// How long a socket may sit idle in a read or a write.
pub const IO_TIMEOUT: Duration = Duration::from_secs(15);

/// Accepts connections forever. `tls` is `None` only for loopback testing
/// (`--no-tls`); the caller enforces that.
pub fn serve(listener: &TcpListener, ctx: &Arc<Ctx>, tls: Option<&Arc<ServerConfig>>) {
    let active = Arc::new(AtomicUsize::new(0));
    for stream in listener.incoming() {
        let stream = match stream {
            Ok(stream) => stream,
            Err(e) => {
                eprintln!("{} accept failed: {e}", now());
                continue;
            }
        };
        if active.fetch_add(1, Ordering::SeqCst) >= MAX_CONNECTIONS {
            active.fetch_sub(1, Ordering::SeqCst);
            // Dropping the stream closes it: too busy to answer politely.
            continue;
        }
        let slot = Slot(Arc::clone(&active));
        let ctx = Arc::clone(ctx);
        let tls = tls.map(Arc::clone);
        let spawned = std::thread::Builder::new()
            .name("serve-conn".into())
            .spawn(move || {
                let _slot = slot;
                connection(stream, &ctx, tls);
            });
        if let Err(e) = spawned {
            eprintln!("{} could not start a connection thread: {e}", now());
        }
    }
}

/// Releases a connection slot when the connection's thread ends, however
/// it ends.
struct Slot(Arc<AtomicUsize>);

impl Drop for Slot {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

fn connection(stream: TcpStream, ctx: &Ctx, tls: Option<Arc<ServerConfig>>) {
    let peer = stream
        .peer_addr()
        .map_or_else(|_| "?".to_owned(), |a: SocketAddr| a.ip().to_string());
    // Without timeouts a client that never sends would hold its thread
    // forever; failing to set them is not worth serving without them.
    if stream.set_read_timeout(Some(IO_TIMEOUT)).is_err()
        || stream.set_write_timeout(Some(IO_TIMEOUT)).is_err()
    {
        return;
    }
    let Some(config) = tls else {
        let mut plain = stream;
        if let Some(line) = exchange(&mut plain, ctx, &peer) {
            eprintln!("{line}");
        }
        return;
    };
    let Ok(conn) = ServerConnection::new(config) else {
        return;
    };
    let mut tls_stream = StreamOwned::new(conn, stream);
    if let Some(line) = exchange(&mut tls_stream, ctx, &peer) {
        eprintln!("{line}");
    }
    tls_stream.conn.send_close_notify();
    // The response is already written; a failed close_notify only means the
    // client hung up first.
    let _ = tls_stream.flush();
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
