//! Tests for `config`: the one beagle folder and its fallbacks.
#![allow(clippy::unwrap_used)] // panicking is the correct failure mode in tests

use std::path::{Path, PathBuf};

use super::{db_path_from, load_from, pick_config, DIRS};

/// A fresh directory under the system temp dir, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("tandem-config-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn the_beagle_folder_comes_first_and_the_old_yaml_still_works() {
    assert_eq!(DIRS.env_var(), "BEAGLE_TANDEM_ROOT");
    let home = Scratch::new();
    let new = home.path().join("beagle/tandem.toml");
    let old = home.path().join("old/config.yaml");
    std::fs::create_dir_all(old.parent().unwrap()).unwrap();
    std::fs::write(&old, "github_user: from-yaml\n").unwrap();
    assert_eq!(pick_config(home.path(), Some(old.clone())), old);
    assert_eq!(load_from(&old).0.github_user.as_deref(), Some("from-yaml"));
    std::fs::create_dir_all(new.parent().unwrap()).unwrap();
    std::fs::write(&new, "github_user = \"from-toml\"\n").unwrap();
    assert_eq!(pick_config(home.path(), Some(old)), new);
    assert_eq!(load_from(&new).0.github_user.as_deref(), Some("from-toml"));
}

#[test]
fn a_broken_config_warns_and_the_defaults_apply() {
    let tmp = Scratch::new();
    for (name, text) in [("tandem.toml", "github_usr = 1\n"), ("c.yaml", "kick: [\n")] {
        let path = tmp.path().join(name);
        std::fs::write(&path, text).unwrap();
        let (config, warning) = load_from(&path);
        assert!(config.github_user.is_none() && config.kick.is_empty());
        let warning = warning.unwrap();
        assert!(warning.contains(name), "{warning}");
        assert!(warning.contains("using defaults"), "{warning}");
    }
    assert!(load_from(&tmp.path().join("missing.toml")).1.is_none());
}

#[test]
fn the_store_follows_env_then_old_var_then_config_then_data_dir() {
    let data = Some(PathBuf::from("/data"));
    let old = Some(PathBuf::from("/old/db.sqlite3"));
    assert_eq!(
        db_path_from(Some("/env"), old.clone(), Some("/cfg"), data.clone()).unwrap(),
        Path::new("/env/cache.sqlite3")
    );
    assert_eq!(
        db_path_from(None, old, Some("/cfg"), data.clone()).unwrap(),
        Path::new("/old/db.sqlite3")
    );
    assert_eq!(
        db_path_from(None, None, Some("/cfg"), data.clone()).unwrap(),
        Path::new("/cfg/cache.sqlite3")
    );
    assert_eq!(
        db_path_from(None, None, None, data).unwrap(),
        Path::new("/data/tandem-tui/cache.sqlite3")
    );
    assert!(db_path_from(None, None, None, None).is_err());
}
