//! Tests for `error`: the IPC contract is that every error serializes to a
//! bare JSON string equal to its message.
#![allow(clippy::expect_used)] // panicking is the correct failure mode in tests

use std::path::PathBuf;

use super::*;

fn json(err: &Error) -> String {
    serde_json::to_string(err).expect("serialize")
}

#[test]
fn serializes_to_its_message_string() {
    let err = Error::UnknownSection("bogus.md".to_owned());
    assert_eq!(json(&err), "\"unknown section `bogus.md`\"");
}

#[test]
fn domain_errors_keep_their_message() {
    let err = Error::from(beagle::Error::InvalidId("Bad Id".to_owned()));
    let expected = beagle::Error::InvalidId("Bad Id".to_owned()).to_string();
    assert_eq!(err.to_string(), expected);
    assert_eq!(
        json(&err),
        serde_json::to_string(&expected).expect("serialize")
    );
}

#[test]
fn unreachable_hint_is_optional() {
    let make = |hint| Error::DaemonUnreachable {
        path: PathBuf::from("/run/beagle-agentd.sock"),
        source: io::Error::new(io::ErrorKind::NotFound, "gone"),
        hint,
    };
    assert_eq!(
        make(true).to_string(),
        "daemon not reachable at /run/beagle-agentd.sock (gone); is it running? try `beagle agent start`"
    );
    assert_eq!(
        make(false).to_string(),
        "daemon not reachable at /run/beagle-agentd.sock (gone)"
    );
}

#[test]
fn socket_and_daemon_messages_match_the_wire_strings() {
    let socket = Error::Socket {
        op: "write",
        source: io::Error::new(io::ErrorKind::BrokenPipe, "broken pipe"),
    };
    assert_eq!(socket.to_string(), "socket write: broken pipe");
    assert_eq!(
        Error::Daemon("no such agent".to_owned()).to_string(),
        "no such agent"
    );
    assert_eq!(
        Error::UnexpectedResponse.to_string(),
        "unexpected daemon response"
    );
    assert_eq!(
        Error::MissingStatus.to_string(),
        "daemon response missing status"
    );
}
