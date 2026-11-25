//! The real listener over loopback TLS: clients that abandon, stall or
//! trickle must never lock everyone else out (the server used to close
//! every new connection, silently, once 32 such clients held its slots).

// Tests fail loudly on setup errors; unwrap/expect is the clearest way.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::time::{Duration, Instant};

use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName};
use rustls::{ClientConfig, ClientConnection, RootCertStore, ServerConfig, StreamOwned};

use crate::auth::Token;
use crate::conn::{Connections, Limits};
use crate::router::Ctx;

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

/// Starts `serve` on a loopback port with a fresh self-signed certificate;
/// returns its address, its connection count and a client that trusts it.
fn start(limits: Limits) -> (SocketAddr, Connections, Arc<ClientConfig>) {
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_owned()]).unwrap();
    let der: CertificateDer<'static> = cert.cert.der().clone();
    let key = PrivateKeyDer::Pkcs8(cert.key_pair.serialize_der().into());
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let server = ServerConfig::builder_with_provider(Arc::clone(&provider))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(vec![der.clone()], key)
        .unwrap();
    let mut roots = RootCertStore::empty();
    roots.add(der).unwrap();
    let client = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let conns = Connections::new(limits.max);
    let ctx = Arc::new(
        Ctx::new(
            Token::parse(TOKEN).unwrap(),
            crate::router::tests::unavailable(),
        )
        .with_connections(conns.clone()),
    );
    let server = Arc::new(server);
    let counted = conns.clone();
    std::thread::spawn(move || super::serve(&listener, &ctx, Some(&server), &counted, limits));
    (addr, conns, Arc::new(client))
}

/// `GET /v1/health` over TLS: the whole response, or why it failed.
fn health(addr: SocketAddr, client: &Arc<ClientConfig>) -> std::io::Result<String> {
    let sock = TcpStream::connect(addr)?;
    sock.set_read_timeout(Some(Duration::from_secs(2)))?;
    sock.set_write_timeout(Some(Duration::from_secs(2)))?;
    let name = ServerName::try_from("localhost").unwrap();
    let conn = ClientConnection::new(Arc::clone(client), name).map_err(std::io::Error::other)?;
    let mut tls = StreamOwned::new(conn, sock);
    tls.write_all(b"GET /v1/health HTTP/1.1\r\nHost: localhost\r\n\r\n")?;
    let mut out = String::new();
    tls.read_to_string(&mut out)?;
    Ok(out)
}

/// Waits up to `limit` for `done`.
fn eventually(limit: Duration, mut done: impl FnMut() -> bool) -> bool {
    let until = Instant::now() + limit;
    while Instant::now() < until {
        if done() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

/// A client that starts a TLS record and then sends one byte at a time,
/// each well inside the I/O timeout, for `for_how_long`.
fn trickle(addr: SocketAddr, for_how_long: Duration) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let Ok(mut s) = TcpStream::connect(addr) else {
            return;
        };
        // A handshake record header announcing 512 bytes that never come.
        let _ = s.write_all(&[0x16, 0x03, 0x01, 0x02, 0x00]);
        let until = Instant::now() + for_how_long;
        while Instant::now() < until {
            if s.write_all(&[0]).is_err() {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    })
}

#[test]
fn abandoned_and_stalled_clients_cannot_lock_the_server_out() {
    let limits = Limits {
        max: 4,
        deadline: Duration::from_millis(600),
        io_timeout: Duration::from_millis(300),
    };
    let (addr, conns, client) = start(limits);
    assert!(health(addr, &client)
        .unwrap()
        .starts_with("HTTP/1.1 200 OK"));

    // More than `max` clients the way a phone loses them: connected and
    // abandoned, reset mid-handshake, or left idle.
    let mut idle = Vec::new();
    for i in 0..(3 * limits.max) {
        let mut s = TcpStream::connect(addr).unwrap();
        match i % 3 {
            0 => drop(s),
            1 => {
                let _ = s.write_all(&[0x16, 0x03, 0x01]);
                drop(s);
            }
            _ => idle.push(s),
        }
    }
    assert!(
        eventually(Duration::from_secs(2), || conns.active() == 0),
        "abandoned and idle clients kept their slots ({} open)",
        conns.active()
    );
    // Then `max` tricklers, which no per-read timeout ever catches.
    let tricklers: Vec<_> = (0..limits.max)
        .map(|_| trickle(addr, Duration::from_secs(5)))
        .collect();
    assert!(
        eventually(Duration::from_secs(2), || conns.active() == limits.max),
        "the tricklers never filled the slots ({} open)",
        conns.active()
    );

    // The deadline frees every slot while the tricklers are still at it
    // and the idle sockets are still open.
    let answered = eventually(Duration::from_secs(3), || {
        health(addr, &client).is_ok_and(|r| r.starts_with("HTTP/1.1 200 OK"))
    });
    assert!(answered, "still locked out ({} open)", conns.active());
    assert!(!tricklers.iter().all(std::thread::JoinHandle::is_finished));
    let body = health(addr, &client).unwrap();
    assert!(body.contains("\"connections\":{\"active\":"), "{body}");
    assert!(body.contains("\"max\":4"), "{body}");
    drop(idle);
    assert!(eventually(Duration::from_secs(2), || conns.active() == 0));
}
