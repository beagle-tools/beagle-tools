// Tests fail loudly on setup errors; unwrap/expect is the clearest way.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

use super::*;

#[test]
fn a_missing_file_is_the_default() {
    let dir = tempfile::tempdir().unwrap();
    let config = load(&dir.path().join("serve.toml")).unwrap();
    assert_eq!(config, Config::default());
    assert_eq!(config.port(), DEFAULT_PORT);
}

#[test]
fn save_then_load_round_trips_with_mode_0600() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("beagle/serve.toml");
    let config = Config {
        token: Some("ab".repeat(32)),
        host: Some("mac.example-tailnet.ts.net".into()),
        port: Some(9000),
        addr: Some("100.64.0.1".parse().unwrap()),
        cert: None,
        key: Some(PathBuf::from("/tmp/k.pem")),
        push: Some(PushConfig {
            team_id: "ABC123DEFG".into(),
            key_id: "KEY1234567".into(),
            key: PathBuf::from("~/.config/beagle/serve/AuthKey_KEY1234567.p8"),
        }),
        secrets: Some(SecretsConfig {
            command: "vault export --format json --project \"example\"".into(),
        }),
    };
    save(&path, &config).unwrap();
    assert_eq!(load(&path).unwrap(), config);
    assert_eq!(permission_warning(&path), None);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}

#[cfg(unix)]
#[test]
fn a_world_readable_file_is_warned_about() {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("serve.toml");
    std::fs::write(&path, "port = 1\n").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(permission_warning(&path).unwrap().contains("chmod 600"));
}

#[test]
fn unknown_keys_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("serve.toml");
    std::fs::write(&path, "prot = 1\n").unwrap();
    let err = load(&path).unwrap_err().to_string();
    assert!(err.contains("prot"), "{err}");
}

#[test]
fn debug_redacts_the_token() {
    let config = Config {
        token: Some("cd".repeat(32)),
        ..Config::default()
    };
    assert!(!format!("{config:?}").contains("cdcd"));
}

#[test]
fn cert_paths_default_to_the_host_under_the_dir() {
    let dir = Path::new("/cfg/beagle/serve");
    let (cert, key) = Config::default().cert_paths(dir, "mac.example-tailnet.ts.net");
    assert_eq!(cert, dir.join("mac.example-tailnet.ts.net.crt"));
    assert_eq!(key, dir.join("mac.example-tailnet.ts.net.key"));
    let custom = Config {
        cert: Some("/etc/c.pem".into()),
        ..Config::default()
    };
    assert_eq!(custom.cert_paths(dir, "h").0, Path::new("/etc/c.pem"));
}
