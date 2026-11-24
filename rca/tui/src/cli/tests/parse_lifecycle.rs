//! Tests for parsing the lifecycle commands (`cli`): archive, unarchive,
//! skill, publish/unpublish and handoff. Split from `parse.rs`.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::*;

fn parse(argv: &[&str]) -> Result<Command, String> {
    parse_args(argv.iter().map(ToString::to_string))
}

#[test]
fn archive_parses_id_force_and_root() {
    let cmd = parse(&["archive", "old-rca", "--force", "--root", "/r"]).expect("parse");
    match cmd {
        Command::Archive { root, id, force } => {
            assert_eq!(id.as_str(), "old-rca");
            assert!(force);
            assert_eq!(root, Some(std::path::PathBuf::from("/r")));
        }
        other => panic!("wrong command: {other:?}"),
    }
    assert!(parse(&["archive"]).is_err(), "id is required");
}

#[test]
fn list_parses_the_archived_flag() {
    match parse(&["list", "--archived"]).expect("parse") {
        Command::List { archived, .. } => assert!(archived),
        other => panic!("wrong command: {other:?}"),
    }
    match parse(&["list"]).expect("parse") {
        Command::List { archived, .. } => assert!(!archived),
        other => panic!("wrong command: {other:?}"),
    }
}

#[test]
fn unarchive_parses_id_and_root() {
    match parse(&["unarchive", "old-rca", "--root", "/r"]).expect("parse") {
        Command::Unarchive { root, id } => {
            assert_eq!(id.as_str(), "old-rca");
            assert_eq!(root, Some(std::path::PathBuf::from("/r")));
        }
        other => panic!("wrong command: {other:?}"),
    }
    assert!(parse(&["unarchive"]).is_err(), "id is required");
}

#[test]
fn skill_parses_status_install_and_default() {
    assert!(matches!(
        parse(&["skill"]),
        Ok(Command::Skill {
            action: SkillAction::Status
        })
    ));
    assert!(matches!(
        parse(&["skill", "status"]),
        Ok(Command::Skill {
            action: SkillAction::Status
        })
    ));
    assert!(matches!(
        parse(&["skill", "install"]),
        Ok(Command::Skill {
            action: SkillAction::Install
        })
    ));
    assert!(parse(&["skill", "frobnicate"]).is_err());
    assert!(parse(&["skill", "install", "extra"]).is_err());
}

#[test]
fn publish_and_unpublish_parse_id_and_root() {
    match parse(&["publish", "my-rca", "--root", "/r"]).expect("parse") {
        Command::SetPublished {
            root,
            id,
            published,
        } => {
            assert_eq!(id.as_str(), "my-rca");
            assert!(published);
            assert_eq!(root, Some(std::path::PathBuf::from("/r")));
        }
        other => panic!("wrong command: {other:?}"),
    }
    match parse(&["unpublish", "my-rca"]).expect("parse") {
        Command::SetPublished { published, .. } => assert!(!published),
        other => panic!("wrong command: {other:?}"),
    }
    assert!(parse(&["publish"]).is_err(), "id required");
}

#[test]
fn handoff_parses_id_and_root() {
    match parse(&["handoff", "my-rca", "--root", "/r"]).expect("parse") {
        Command::Handoff { root, id } => {
            assert_eq!(id.as_str(), "my-rca");
            assert_eq!(root, Some(std::path::PathBuf::from("/r")));
        }
        other => panic!("wrong command: {other:?}"),
    }
    assert!(parse(&["handoff"]).is_err(), "id is required");
}
