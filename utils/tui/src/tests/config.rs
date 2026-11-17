//! Tests for `config`.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::*;

const DIRS: AppDirs = AppDirs {
    app: "demo",
    env_var: "DEMO_ROOT",
    data_dir: "items",
};

#[derive(Debug, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct DemoConfig {
    root: Option<String>,
}

#[test]
fn flag_beats_env_beats_config() {
    let cwd = Path::new("/nowhere/at/all");
    let config = Some("/from/config");
    assert_eq!(
        DIRS.resolve_root(Some("/flag"), Some("/env"), config, cwd),
        PathBuf::from("/flag")
    );
    assert_eq!(
        DIRS.resolve_root(None, Some("/env"), config, cwd),
        PathBuf::from("/env")
    );
    assert_eq!(
        DIRS.resolve_root(None, None, config, cwd),
        PathBuf::from("/from/config")
    );
    assert_eq!(DIRS.resolve_root(None, None, None, cwd), cwd);
}

#[test]
fn discovery_finds_the_nearest_ancestor_with_the_data_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let nested = tmp.path().join("a").join("b");
    std::fs::create_dir_all(&nested).unwrap();
    assert_eq!(DIRS.discover_root(&nested), nested);
    std::fs::create_dir(tmp.path().join("items")).unwrap();
    assert_eq!(DIRS.discover_root(&nested), tmp.path());
}

#[test]
fn config_file_is_optional_and_strict() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("config.toml");
    assert_eq!(
        load_toml::<DemoConfig>(&path).unwrap(),
        DemoConfig::default()
    );
    std::fs::write(&path, "root = \"/x\"").unwrap();
    assert_eq!(
        load_toml::<DemoConfig>(&path).unwrap().root.as_deref(),
        Some("/x")
    );
    std::fs::write(&path, "rooot = \"/x\"").unwrap();
    assert!(matches!(
        load_toml::<DemoConfig>(&path),
        Err(ConfigError::Parse { .. })
    ));
}

#[test]
fn config_path_is_per_app() {
    if let Some(path) = DIRS.config_path() {
        assert!(path.ends_with("demo/config.toml"));
    }
}

#[test]
fn tilde_paths_that_are_not_home_are_left_alone() {
    assert_eq!(expand_tilde("~bob/x"), PathBuf::from("~bob/x"));
    assert_eq!(expand_tilde("/abs"), PathBuf::from("/abs"));
}
