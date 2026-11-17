//! Tests for `config`.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::*;

#[test]
fn flag_beats_env_beats_config() {
    let config = Config {
        root: Some("/from/config".to_owned()),
    };
    let cwd = Path::new("/nowhere/at/all");
    assert_eq!(
        resolve_root(Some("/flag"), Some("/env"), &config, cwd),
        PathBuf::from("/flag")
    );
    assert_eq!(
        resolve_root(None, Some("/env"), &config, cwd),
        PathBuf::from("/env")
    );
    assert_eq!(
        resolve_root(None, None, &config, cwd),
        PathBuf::from("/from/config")
    );
    assert_eq!(resolve_root(None, None, &Config::default(), cwd), cwd);
}

#[test]
fn config_file_is_optional_and_strict() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("config.toml");
    assert_eq!(load(&path).unwrap(), Config::default());
    std::fs::write(&path, "root = \"/x\"").unwrap();
    assert_eq!(load(&path).unwrap().root.as_deref(), Some("/x"));
    std::fs::write(&path, "rooot = \"/x\"").unwrap();
    assert!(load(&path).is_err());
}

#[test]
fn tilde_paths_that_are_not_home_are_left_alone() {
    assert_eq!(expand_tilde("~bob/x"), PathBuf::from("~bob/x"));
    assert_eq!(expand_tilde("/abs"), PathBuf::from("/abs"));
}
