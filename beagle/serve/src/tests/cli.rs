// Tests fail loudly on setup errors; unwrap is the clearest way.
#![allow(clippy::unwrap_used)]

use super::*;

fn p(args: &[&str]) -> Result<Command, String> {
    parse(args.iter().map(OsString::from).collect())
}

#[test]
fn no_arguments_serves_with_defaults() {
    assert_eq!(p(&[]).unwrap(), Command::Serve(ServeOptions::default()));
}

#[test]
fn every_server_option_parses() {
    let cmd = p(&[
        "--addr",
        "127.0.0.1",
        "--port",
        "9000",
        "--any-addr",
        "--no-tls",
        "--cert",
        "c",
        "--key",
        "k",
    ])
    .unwrap();
    assert_eq!(
        cmd,
        Command::Serve(ServeOptions {
            addr: Some("127.0.0.1".parse().unwrap()),
            port: Some(9000),
            any_addr: true,
            cert: Some("c".into()),
            key: Some("k".into()),
            no_tls: true,
        })
    );
}

#[test]
fn pair_and_the_service_commands() {
    assert_eq!(p(&["pair"]).unwrap(), Command::Pair(PairOptions::default()));
    assert_eq!(
        p(&[
            "pair",
            "--rotate",
            "--host",
            "mac.example-tailnet.ts.net",
            "--port",
            "9443"
        ])
        .unwrap(),
        Command::Pair(PairOptions {
            rotate: true,
            host: Some("mac.example-tailnet.ts.net".into()),
            port: Some(9443),
        })
    );
    assert_eq!(p(&["install"]).unwrap(), Command::Install);
    assert_eq!(p(&["uninstall"]).unwrap(), Command::Uninstall);
    assert_eq!(p(&["status"]).unwrap(), Command::Status);
    assert!(p(&["status", "now"]).unwrap_err().contains("unexpected"));
    assert!(p(&["pair", "--addr", "x"]).unwrap_err().contains("unknown"));
}

#[test]
fn help_and_mistakes() {
    assert_eq!(p(&["--help"]).unwrap(), Command::Help);
    assert_eq!(p(&["help"]).unwrap(), Command::Help);
    assert_eq!(p(&["pair", "-h"]).unwrap(), Command::Help);
    assert!(p(&["--addr"]).unwrap_err().contains("needs a value"));
    assert!(p(&["--addr", "mac"]).unwrap_err().contains("not an IP"));
    assert!(p(&["--port", "0"]).unwrap_err().contains("not a port"));
    assert!(p(&["--port", "70000"]).unwrap_err().contains("not a port"));
    assert!(p(&["--bogus"]).unwrap_err().contains("unknown argument"));
}

#[test]
fn secrets_commands() {
    let cmd = "vault export --format json";
    assert_eq!(
        p(&["secrets", "sync", "--from", cmd]).unwrap(),
        Command::Secrets(SecretsCommand::Sync {
            from: Some(cmd.into())
        })
    );
    assert_eq!(
        p(&["secrets", "sync"]).unwrap(),
        Command::Secrets(SecretsCommand::Sync { from: None })
    );
    assert_eq!(
        p(&["secrets", "status"]).unwrap(),
        Command::Secrets(SecretsCommand::Status)
    );
    assert_eq!(
        p(&["secrets", "clear"]).unwrap(),
        Command::Secrets(SecretsCommand::Clear)
    );
    let err = |args: &[&str]| p(args).unwrap_err();
    assert!(err(&["secrets", "sync", "--from"]).contains("needs a command"));
    assert!(err(&["secrets", "sync", "--from", " "]).contains("needs a command"));
    assert!(err(&["secrets", "status", "--from", "x"]).contains("unknown argument"));
    assert!(err(&["secrets", "show"]).contains("unknown secrets command"));
    assert!(err(&["secrets"]).contains("needs a command"));
}
