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
fn the_beagle_folder_comes_first_and_the_old_paths_still_work() {
    assert_eq!(DIRS.env_var(), "BEAGLE_STANDUP_ROOT");
    assert_eq!(DIRS.legacy_env, ["STANDUP_ROOT"]);
    let home = tempfile::tempdir().unwrap();
    let old = home.path().join("standup/config.toml");
    std::fs::create_dir_all(old.parent().unwrap()).unwrap();
    std::fs::write(&old, "root = \"/old\"").unwrap();
    assert_eq!(DIRS.config_file_in(home.path()), old);
    let new = home.path().join("beagle/standup.toml");
    std::fs::create_dir_all(new.parent().unwrap()).unwrap();
    std::fs::write(&new, "root = \"/new\"").unwrap();
    assert_eq!(DIRS.config_file_in(home.path()), new);
    assert_eq!(load(&new).unwrap().root.as_deref(), Some("/new"));
}

#[test]
fn a_broken_config_warns_and_the_tool_still_starts() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("standup.toml");
    std::fs::write(&path, "rooot = \"/x\"").unwrap();
    let (config, warning) = beagle_tui::config::load_toml_or_default::<Config>(&path);
    assert_eq!(config, Config::default());
    assert!(warning.unwrap().contains("using defaults"));
}

#[test]
fn tilde_paths_that_are_not_home_are_left_alone() {
    assert_eq!(expand_tilde("~bob/x"), PathBuf::from("~bob/x"));
    assert_eq!(expand_tilde("/abs"), PathBuf::from("/abs"));
}
