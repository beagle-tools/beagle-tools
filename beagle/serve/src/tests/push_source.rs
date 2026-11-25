// Tests fail loudly on setup errors; unwrap/expect is the clearest way.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use super::*;
use crate::secrets::tests_support::{p8, MemoryStore};

const KEY_ID: &str = "KEY1234567";
const TEAM_ID: &str = "ABC123DEFG";

fn env(pairs: &[(&str, String)]) -> impl Fn(&str) -> Option<String> {
    let map: BTreeMap<String, String> = pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), v.clone()))
        .collect();
    move |name| map.get(name).cloned()
}

fn full_env(pem: &str) -> impl Fn(&str) -> Option<String> {
    env(&[
        (ENV_KEY, pem.to_owned()),
        (ENV_KEY_ID, KEY_ID.to_owned()),
        (ENV_TEAM_ID, TEAM_ID.to_owned()),
    ])
}

/// A `[push]` pointing at a real key file in `dir`.
fn file_config(dir: &std::path::Path) -> Config {
    let key = dir.join("AuthKey.p8");
    beagle_utils::fs::write_atomic_with_mode(&key, p8().as_bytes(), 0o600).unwrap();
    Config {
        push: Some(PushConfig {
            team_id: "FILETEAM01".into(),
            key_id: "FILEKEY001".into(),
            key,
        }),
        ..Config::default()
    }
}

#[test]
fn nothing_set_means_push_is_off() {
    let loaded = resolve(&Config::default(), &env(&[]), &MemoryStore::default()).unwrap();
    assert!(loaded.is_none());
}

#[test]
fn a_full_environment_loads_the_key() {
    let loaded = resolve(
        &Config::default(),
        &full_env(&p8()),
        &MemoryStore::default(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(loaded.source, Source::Env);
    assert_eq!(loaded.key_id, KEY_ID);
    assert!(loaded.summary().contains("environment"));
}

#[test]
fn a_one_line_pem_with_escaped_newlines_loads() {
    let pem = p8().replace('\n', "\\n");
    let loaded = resolve(&Config::default(), &full_env(&pem), &MemoryStore::default()).unwrap();
    assert!(loaded.is_some());
}

#[test]
fn a_partial_environment_names_what_is_missing() {
    let why = resolve(
        &Config::default(),
        &env(&[(ENV_KEY_ID, KEY_ID.to_owned())]),
        &MemoryStore::default(),
    )
    .unwrap_err();
    assert!(why.contains(ENV_KEY) && why.contains(ENV_TEAM_ID), "{why}");
}

#[test]
fn a_bad_key_is_an_error_that_does_not_echo_the_key() {
    let secret = "-----BEGIN PRIVATE KEY-----\nTOPSECRETbutbroken\n-----END PRIVATE KEY-----";
    let why = resolve(
        &Config::default(),
        &full_env(secret),
        &MemoryStore::default(),
    )
    .unwrap_err();
    assert!(why.starts_with(ENV_KEY), "{why}");
    assert!(!why.contains("TOPSECRET"), "{why}");
}

#[test]
fn a_bad_id_is_an_error() {
    let bad = env(&[
        (ENV_KEY, p8()),
        (ENV_KEY_ID, "short".to_owned()),
        (ENV_TEAM_ID, TEAM_ID.to_owned()),
    ]);
    let why = resolve(&Config::default(), &bad, &MemoryStore::default()).unwrap_err();
    assert!(why.contains("key_id"), "{why}");
}

#[test]
fn the_environment_wins_over_the_keychain_and_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let store = MemoryStore::full(&p8(), "KCKEY00001", "KCTEAM0001");
    let loaded = resolve(&file_config(dir.path()), &full_env(&p8()), &store)
        .unwrap()
        .unwrap();
    assert_eq!(loaded.source, Source::Env);
}

#[test]
fn the_keychain_wins_over_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let store = MemoryStore::full(&p8(), "KCKEY00001", "KCTEAM0001");
    let loaded = resolve(&file_config(dir.path()), &env(&[]), &store)
        .unwrap()
        .unwrap();
    assert_eq!(loaded.source, Source::Keychain);
    assert_eq!(loaded.key_id, "KCKEY00001");
    assert!(loaded.summary().contains("Keychain"));
}

#[test]
fn the_file_is_used_when_nothing_else_is_set() {
    let dir = tempfile::tempdir().unwrap();
    let loaded = resolve(&file_config(dir.path()), &env(&[]), &MemoryStore::default())
        .unwrap()
        .unwrap();
    assert_eq!(loaded.source, Source::File);
    assert_eq!(loaded.key_id, "FILEKEY001");
}

#[test]
fn a_broken_keychain_turns_push_off_instead_of_falling_through() {
    let dir = tempfile::tempdir().unwrap();
    let store = MemoryStore::default();
    crate::secrets::SecretStore::set(&store, ENV_KEY_ID, "KCKEY00001").unwrap();
    let why = resolve(&file_config(dir.path()), &env(&[]), &store).unwrap_err();
    assert!(why.contains("Keychain"), "{why}");
}
