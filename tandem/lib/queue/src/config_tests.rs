//! Tests for `config`: the one beagle folder and its fallbacks.
#![allow(clippy::unwrap_used)] // panicking is the correct failure mode in tests

use std::path::{Path, PathBuf};

use super::{db_path_from, load_from, pick_config, save_github_user_to, DIRS};

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

#[test]
fn saving_the_account_rewrites_only_its_line() {
    let tmp = Scratch::new();
    let toml = tmp.path().join("tandem.toml");
    std::fs::write(
        &toml,
        "# mine\ngithub_user = \"octo-work\"\nkick = [\"agentctl\", \"run\"]\n",
    )
    .unwrap();
    save_github_user_to(&toml, "octo-personal").unwrap();
    assert_eq!(
        std::fs::read_to_string(&toml).unwrap(),
        "# mine\ngithub_user = \"octo-personal\"\nkick = [\"agentctl\", \"run\"]\n"
    );
    let loaded = load_from(&toml).0;
    assert_eq!(loaded.github_user.as_deref(), Some("octo-personal"));
    assert_eq!(loaded.kick, ["agentctl", "run"]);

    // No line yet: added first, before anything else.
    std::fs::write(&toml, "root = \"/tmp/x\"\n").unwrap();
    save_github_user_to(&toml, "octo-work").unwrap();
    assert_eq!(load_from(&toml).0.github_user.as_deref(), Some("octo-work"));
    assert_eq!(load_from(&toml).0.root.as_deref(), Some("/tmp/x"));

    // The older YAML file keeps YAML.
    let yaml = tmp.path().join("config.yaml");
    std::fs::write(&yaml, "github_user: octo-work\nkick: [a]\n").unwrap();
    save_github_user_to(&yaml, "octo-personal").unwrap();
    assert_eq!(
        std::fs::read_to_string(&yaml).unwrap(),
        "github_user: octo-personal\nkick: [a]\n"
    );

    // No file: created, with its folder.
    let fresh = tmp.path().join("new/beagle/tandem.toml");
    save_github_user_to(&fresh, "octo-solo").unwrap();
    assert_eq!(
        load_from(&fresh).0.github_user.as_deref(),
        Some("octo-solo")
    );
}

#[test]
fn saving_leaves_a_broken_file_alone_and_rejects_non_logins() {
    let tmp = Scratch::new();
    let toml = tmp.path().join("tandem.toml");
    std::fs::write(&toml, "github_usr = 1\n").unwrap();
    let err = save_github_user_to(&toml, "octo-work").unwrap_err();
    assert!(err.contains("does not parse"), "{err}");
    assert_eq!(std::fs::read_to_string(&toml).unwrap(), "github_usr = 1\n");
    // Nothing left behind but the file itself.
    assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 1);
    for bad in ["", "octo work", "x\"\nkick = 1"] {
        assert!(save_github_user_to(&toml, bad).is_err(), "{bad:?}");
    }
}
