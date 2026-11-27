//! Tests for the exposure checks, over a fake `/etc/ssh` and canned
//! command output (never this machine's).
#![allow(clippy::unwrap_used)] // panicking is the correct failure mode in tests

use std::fs;

use super::*;

const SSHD_ON: &str = "\t\"com.openssh.sshd\" => enabled\n";

/// A fake `/etc/ssh` with `sshd_config` and `sshd_config.d/<name>` files.
fn etc_ssh(main: &str, dropins: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("sshd_config"), main).unwrap();
    fs::create_dir_all(dir.path().join("sshd_config.d")).unwrap();
    for (name, text) in dropins {
        fs::write(dir.path().join("sshd_config.d").join(name), text).unwrap();
    }
    dir
}

fn inputs(etc: &Path) -> Inputs {
    Inputs {
        etc_ssh: etc.to_path_buf(),
        launchd_disabled: None,
        listeners: None,
        stealth: None,
        mdm: None,
        token_created: None,
        now: SystemTime::UNIX_EPOCH + Duration::from_secs(1_000 * 86_400),
    }
}

#[test]
fn on_an_mdm_managed_mac_firewall_and_remote_management_go_to_it() {
    let enrolled = "Enrolled via DEP: Yes\nMDM enrollment: Yes (User Approved)\n";
    assert!(mdm_enrolled(enrolled));
    assert!(!mdm_enrolled("Enrolled via DEP: No\nMDM enrollment: No\n"));
    let etc = etc_ssh("", &[]);
    let w = warnings(&Inputs {
        listeners: Some("p1\ncARDAgent\nf1\nn*:3283\n".to_owned()),
        stealth: Some("Firewall stealth mode is off".to_owned()),
        mdm: Some(enrolled.to_owned()),
        ..inputs(etc.path())
    });
    assert_eq!(w.len(), 2, "{w:#?}");
    for warning in &w {
        assert!(warning.contains("managed by MDM"), "{warning}");
        assert!(!warning.contains("sudo") && !warning.contains("System Settings"));
    }
}

const STOCK: &str = "Include /etc/ssh/sshd_config.d/*\n#PasswordAuthentication yes\nUsePAM yes\n";

#[test]
fn stock_sshd_accepts_passwords_by_default() {
    let etc = etc_ssh(STOCK, &[]);
    let auth = sshd::password_auth(etc.path());
    assert_eq!(auth.password.from, "the default");
    assert_eq!(
        auth.password_methods(),
        ["password", "keyboard-interactive"]
    );
    let w = warnings(&Inputs {
        launchd_disabled: Some(SSHD_ON.to_owned()),
        ..inputs(etc.path())
    });
    assert_eq!(w.len(), 1, "{w:?}");
    assert!(w[0].contains("PasswordAuthentication no"), "{w:?}");
}

#[test]
fn the_first_value_read_wins_in_include_order() {
    let etc = etc_ssh(
        &format!("{STOCK}PasswordAuthentication yes\nKbdInteractiveAuthentication yes\n"),
        &[
            ("00-keys.conf", "PasswordAuthentication no\n"),
            ("10-later.conf", "PasswordAuthentication yes\n"),
            ("20-kbd.conf", "ChallengeResponseAuthentication=no\n"),
            ("notes.txt", "PasswordAuthentication yes\n"),
        ],
    );
    let auth = sshd::password_auth(etc.path());
    assert_eq!(auth.password.value, "no");
    assert!(auth.password.from.ends_with("00-keys.conf:1"), "{auth:?}");
    assert_eq!(auth.keyboard_interactive.value, "no");
    assert_eq!(auth.password_methods(), [] as [&str; 0]);
    let w = warnings(&Inputs {
        launchd_disabled: Some(SSHD_ON.to_owned()),
        ..inputs(etc.path())
    });
    assert!(w.is_empty(), "{w:?}");
}

#[test]
fn a_value_before_the_include_beats_the_dropins() {
    let etc = etc_ssh(
        "passwordauthentication YES\nInclude sshd_config.d/*.conf\n",
        &[("00-keys.conf", "PasswordAuthentication no\n")],
    );
    let auth = sshd::password_auth(etc.path());
    assert_eq!(auth.password.value, "yes");
    assert!(auth.password.from.ends_with("sshd_config:1"), "{auth:?}");
}

#[test]
fn match_blocks_and_authentication_methods() {
    let etc = etc_ssh(
        "AuthenticationMethods publickey\nMatch User guest\n  PasswordAuthentication no\n",
        &[],
    );
    let auth = sshd::password_auth(etc.path());
    // The Match line isn't global; AuthenticationMethods alone shuts passwords out.
    assert_eq!(auth.password.from, "the default");
    assert_eq!(auth.password_methods(), [] as [&str; 0]);
}

#[test]
fn ssh_is_not_checked_while_remote_login_is_off() {
    let etc = etc_ssh(STOCK, &[]);
    let off = "\t\"com.openssh.sshd\" => disabled\n";
    let w = warnings(&Inputs {
        launchd_disabled: Some(off.to_owned()),
        ..inputs(etc.path())
    });
    assert!(w.is_empty(), "{w:?}");
}

#[test]
fn launchd_states_in_both_spellings() {
    let list = "disabled services = {\n\t\"com.apple.screensharing\" => false\n\t\"com.example.x\" => true\n}\n";
    assert_eq!(service_enabled(list, "com.apple.screensharing"), Some(true));
    assert_eq!(service_enabled(list, "com.example.x"), Some(false));
    assert_eq!(service_enabled(list, "com.openssh.sshd"), None);
}

/// `lsof -F cn` output for a few made-up processes.
const LSOF: &str = "p10\ncARDAgent\nf9\nn*:3283\n\
p20\ncpostgres\nf5\nn*:5432\nf6\nn[::]:5432\n\
p30\ncbeagle\nf3\nn100.64.0.1:8787\nf4\nn[fd7a:115c:a1e0::1]:8787\n\
p40\ncnode\nf7\nn127.0.0.1:3000\nf8\nn[::1]:3000\n\
p50\ncwebapp\nf2\nn192.0.2.10:8080\n";

#[test]
fn listeners_skip_loopback_and_tailscale() {
    assert_eq!(
        exposed_listeners(LSOF),
        [
            ("ARDAgent".to_owned(), 3283),
            ("postgres".to_owned(), 5432),
            ("webapp".to_owned(), 8080),
        ]
    );
}

#[test]
fn every_warning_names_its_fix() {
    let etc = etc_ssh(STOCK, &[]);
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000 * 86_400);
    let w = warnings(&Inputs {
        launchd_disabled: Some(format!(
            "{SSHD_ON}\t\"com.apple.screensharing\" => enabled\n"
        )),
        listeners: Some(LSOF.to_owned()),
        stealth: Some("Firewall stealth mode is off\n".to_owned()),
        token_created: Some(now - Duration::from_secs(120 * 86_400)),
        now,
        ..inputs(etc.path())
    });
    assert_eq!(w.len(), 7, "{w:#?}");
    assert!(w[0].starts_with("Remote Login (SSH) accepts password"));
    assert!(w[1].contains("Remote Management"));
    assert!(w[2].contains("`postgres` listens on port 5432"));
    assert!(w[3].contains("`webapp` listens on port 8080"));
    assert!(w[4].starts_with("Screen Sharing is on"));
    assert!(w[5].contains("--setstealthmode on"));
    assert!(w[6].contains("120 days old") && w[6].contains("--rotate"));
}

#[test]
fn stealth_output_variants() {
    assert_eq!(stealth_on("Firewall stealth mode is on"), Some(true));
    assert_eq!(stealth_on("Stealth mode enabled"), Some(true));
    assert_eq!(stealth_on("Stealth mode disabled"), Some(false));
    assert_eq!(stealth_on(""), None);
}

#[test]
fn unknown_or_young_tokens_do_not_warn() {
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000 * 86_400);
    assert_eq!(token_age_days(None, now), None);
    assert_eq!(
        token_age_days(Some(now + Duration::from_secs(60)), now),
        None
    );
    let etc = etc_ssh("", &[]);
    let young = warnings(&Inputs {
        token_created: Some(now - Duration::from_secs(TOKEN_MAX_DAYS * 86_400)),
        ..inputs(etc.path())
    });
    assert!(young.is_empty(), "{young:?}");
}
