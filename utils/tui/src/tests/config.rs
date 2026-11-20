//! Tests for `config`: the rule-15 order and its fallbacks.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::*;

const DIRS: AppDirs = AppDirs {
    app: "demo",
    legacy_env: &["DEMO_ROOT"],
    legacy_configs: &["demo/config.toml"],
    marker: Some(".demo-store"),
    data_dir: Some("items"),
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
        DIRS.find_root(None, None, config, cwd),
        Some((PathBuf::from("/from/config"), RootSource::Config))
    );
    assert_eq!(DIRS.resolve_root(None, None, None, cwd), cwd);
}

#[test]
fn a_marker_beats_the_config_but_not_the_env() {
    let tmp = tempfile::tempdir().unwrap();
    let deep = tmp.path().join("a/b");
    std::fs::create_dir_all(&deep).unwrap();
    std::fs::write(tmp.path().join(".demo-store"), "").unwrap();
    assert_eq!(
        DIRS.find_root(None, None, Some("/cfg"), &deep),
        Some((tmp.path().to_path_buf(), RootSource::Marker))
    );
    assert_eq!(
        DIRS.find_root(None, Some("/env"), Some("/cfg"), &deep),
        Some((PathBuf::from("/env"), RootSource::Env))
    );
}

#[test]
fn a_directory_named_like_the_marker_does_not_count() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir(tmp.path().join(".demo-store")).unwrap();
    assert_eq!(DIRS.marked_ancestor(tmp.path()), None);
}

#[test]
fn discovery_finds_the_nearest_ancestor_with_the_data_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let nested = tmp.path().join("a").join("b");
    std::fs::create_dir_all(&nested).unwrap();
    assert_eq!(DIRS.discover_root(&nested), nested);
    assert_eq!(DIRS.find_root(None, None, None, &nested), None);
    std::fs::create_dir(tmp.path().join("items")).unwrap();
    assert_eq!(DIRS.discover_root(&nested), tmp.path());
    assert_eq!(
        DIRS.find_root(None, None, None, &nested),
        Some((tmp.path().to_path_buf(), RootSource::DataDir))
    );
}

#[test]
fn the_env_var_is_named_after_the_tool() {
    assert_eq!(DIRS.env_var(), "BEAGLE_DEMO_ROOT");
    let dashed = AppDirs {
        app: "tandem-tui",
        ..DIRS
    };
    assert_eq!(dashed.env_var(), "BEAGLE_TANDEM_TUI_ROOT");
}

#[test]
fn the_beagle_config_wins_and_the_old_one_is_a_fallback() {
    let home = tempfile::tempdir().unwrap();
    let new = home.path().join("beagle/demo.toml");
    let old = home.path().join("demo/config.toml");
    // Neither exists: the new path (where a new file is written).
    assert_eq!(DIRS.config_file_in(home.path()), new);
    std::fs::create_dir_all(old.parent().unwrap()).unwrap();
    std::fs::write(&old, "").unwrap();
    assert_eq!(DIRS.config_file_in(home.path()), old);
    std::fs::create_dir_all(new.parent().unwrap()).unwrap();
    std::fs::write(&new, "").unwrap();
    assert_eq!(DIRS.config_file_in(home.path()), new);
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
fn a_broken_config_warns_and_gives_the_defaults() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("demo.toml");
    std::fs::write(&path, "rooot = \"/x\"").unwrap();
    let (config, warning) = load_toml_or_default::<DemoConfig>(&path);
    assert_eq!(config, DemoConfig::default());
    let warning = warning.unwrap();
    assert!(warning.contains("demo.toml"), "{warning}");
    assert!(warning.contains("using defaults"), "{warning}");
    std::fs::write(&path, "root = \"/x\"").unwrap();
    assert_eq!(load_toml_or_default::<DemoConfig>(&path).1, None);
}

#[test]
fn config_path_is_per_tool_under_beagle() {
    assert_eq!(
        DIRS.config_path_in(Path::new("/c")),
        PathBuf::from("/c/beagle/demo.toml")
    );
}

#[test]
fn tilde_paths_that_are_not_home_are_left_alone() {
    assert_eq!(expand_tilde("~bob/x"), PathBuf::from("~bob/x"));
    assert_eq!(expand_tilde("/abs"), PathBuf::from("/abs"));
}
