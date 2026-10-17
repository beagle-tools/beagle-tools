//! Tests for the base command-line parser.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use std::ffi::OsString;

use super::*;
use crate::registry::REGISTRY;

fn p(args: &[&str]) -> Result<Command, String> {
    parse(args.iter().map(OsString::from).collect(), REGISTRY)
}

fn ext(args: &[&str]) -> ExtCommand {
    match p(args).unwrap() {
        Command::Ext(c) => c,
        other => panic!("{args:?} parsed as {other:?}"),
    }
}

#[test]
fn bare_and_help_forms_print_help() {
    for args in [&[][..], &["help"], &["--help"], &["-h"]] {
        assert_eq!(p(args).unwrap(), Command::Help { ext: None }, "{args:?}");
    }
    assert_eq!(
        p(&["help", "rca"]).unwrap(),
        Command::Help {
            ext: Some("rca".into())
        }
    );
}

#[test]
fn help_anywhere_in_a_base_command_wins() {
    for args in [
        &["extensions", "install", "rca", "--help"][..],
        &["ext", "list", "-h"],
        &["update", "--help"],
        &["version", "--help"],
    ] {
        assert_eq!(p(args).unwrap(), Command::Help { ext: None }, "{args:?}");
    }
}

#[test]
fn version_doctor_and_self_update_parse() {
    for args in [&["version"][..], &["--version"], &["-V"]] {
        assert_eq!(p(args).unwrap(), Command::Version);
    }
    assert_eq!(p(&["doctor"]).unwrap(), Command::Doctor);
    assert_eq!(
        p(&["update", "--version", "0.2.0", "--all"]).unwrap(),
        Command::SelfUpdate {
            version: Some("0.2.0".into()),
            all: true
        }
    );
    assert_eq!(
        p(&["update", "--version=v0.2.0"]).unwrap(),
        Command::SelfUpdate {
            version: Some("v0.2.0".into()),
            all: false
        }
    );
}

#[test]
fn every_extensions_command_parses() {
    assert_eq!(ext(&["extensions", "list"]), ExtCommand::List);
    assert_eq!(
        ext(&["ext", "available", "--pre"]),
        ExtCommand::Available { pre: true }
    );
    assert_eq!(
        ext(&["ext", "install", "rca", "--version", "0.29.0", "-y"]),
        ExtCommand::Install {
            name: "rca".into(),
            version: Some("0.29.0".into()),
            yes: true,
            pre: false
        }
    );
    assert_eq!(
        ext(&["ext", "update", "rca", "agentd", "--yes"]),
        ExtCommand::Update {
            names: vec!["rca".into(), "agentd".into()],
            all: false,
            version: None,
            yes: true
        }
    );
    assert_eq!(
        ext(&["ext", "update", "--all"]),
        ExtCommand::Update {
            names: vec![],
            all: true,
            version: None,
            yes: false
        }
    );
    assert_eq!(
        ext(&["ext", "use", "rca", "0.29.0"]),
        ExtCommand::Use {
            name: "rca".into(),
            version: "0.29.0".into()
        }
    );
    assert_eq!(
        ext(&["ext", "uninstall", "rca", "--all-versions"]),
        ExtCommand::Uninstall {
            name: "rca".into(),
            all_versions: true
        }
    );
    assert_eq!(
        ext(&["ext", "prune"]),
        ExtCommand::Prune {
            name: None,
            keep: 1
        }
    );
    assert_eq!(
        ext(&["ext", "prune", "rca", "--keep", "0"]),
        ExtCommand::Prune {
            name: Some("rca".into()),
            keep: 0
        }
    );
    assert_eq!(
        ext(&["ext", "path", "rca"]),
        ExtCommand::Path { name: "rca".into() }
    );
}

#[test]
fn ext_update_is_intercepted_for_registry_extensions_only() {
    assert_eq!(
        ext(&["rca", "update", "--version", "0.28.0"]),
        ExtCommand::Update {
            names: vec!["rca".into()],
            all: false,
            version: Some("0.28.0".into()),
            yes: false
        }
    );
    // Not a registry name: forwarded like anything else.
    assert_eq!(
        p(&["fake", "update"]).unwrap(),
        Command::Run {
            name: "fake".into(),
            args: vec!["update".into()]
        }
    );
    // `update` not in first position is the extension's business.
    assert_eq!(
        p(&["rca", "list", "update"]).unwrap(),
        Command::Run {
            name: "rca".into(),
            args: vec!["list".into(), "update".into()]
        }
    );
}

#[test]
fn extension_args_are_forwarded_verbatim() {
    let args = vec!["--", "", "--help", "-x", "a b"];
    let Command::Run {
        name,
        args: forwarded,
    } = p(&[&["rca"][..], &args].concat()).unwrap()
    else {
        panic!("expected Run");
    };
    assert_eq!(name, "rca");
    assert_eq!(
        forwarded,
        args.iter().map(OsString::from).collect::<Vec<_>>()
    );
}

#[cfg(unix)]
#[test]
fn non_utf8_extension_args_survive_byte_for_byte() {
    use std::os::unix::ffi::OsStringExt as _;
    let raw = OsString::from_vec(vec![0x66, 0xff, 0x6f]);
    let parsed = parse(vec!["rca".into(), raw.clone()], REGISTRY).unwrap();
    assert_eq!(
        parsed,
        Command::Run {
            name: "rca".into(),
            args: vec![raw.clone()]
        }
    );
    assert!(
        parse(vec!["extensions".into(), raw], REGISTRY).is_err(),
        "base commands need UTF-8"
    );
}

#[test]
fn mistakes_are_usage_errors() {
    for args in [
        &["--bogus"][..],
        &["extensions"],
        &["extensions", "frobnicate"],
        &["extensions", "install"],
        &["extensions", "install", "rca", "--bogus"],
        &["extensions", "list", "--all"],
        &["extensions", "update"],
        &["extensions", "update", "rca", "--all"],
        &[
            "extensions",
            "update",
            "rca",
            "agentd",
            "--version",
            "1.0.0",
        ],
        &["extensions", "use", "rca"],
        &["extensions", "prune", "--keep", "many"],
        &["extensions", "install", "rca", "--version"],
        &["version", "extra"],
        &["update", "--pre"],
        &["rca", "update", "--all"],
        &["completions"],
        &["self"],
    ] {
        assert!(p(args).is_err(), "{args:?} should be rejected");
    }
}
