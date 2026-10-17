//! Tests for `agentd`: the pure plist rendering and status formatting.
#![allow(clippy::expect_used)] // panicking is the correct failure mode in tests

use std::path::PathBuf;

use super::*;

#[test]
fn plist_has_label_program_and_keepalive() {
    let plist = render_plist(&LaunchdConfig {
        label: "com.beagle.agentd".to_string(),
        program: PathBuf::from("/usr/local/bin/beagle-agentd"),
        log_dir: PathBuf::from("/home/me/.local/state/beagle-agent/logs"),
        path: "/opt/homebrew/bin:/usr/bin".to_string(),
    });

    assert!(plist.contains("<string>com.beagle.agentd</string>"));
    assert!(plist.contains("<string>/usr/local/bin/beagle-agentd</string>"));
    // Autostart on login and restart on crash.
    assert!(plist.contains("<key>RunAtLoad</key>\n    <true/>"));
    assert!(plist.contains("<key>KeepAlive</key>\n    <true/>"));
    // Logs point under the state dir.
    assert!(plist.contains("beagle-agent/logs/agentd.out.log"));
    assert!(plist.contains("beagle-agent/logs/agentd.err.log"));
    // Well-formed enough to be a plist.
    assert!(plist.starts_with("<?xml"));
    assert!(plist.trim_end().ends_with("</plist>"));
}

#[test]
fn unit_has_execstart_and_restart() {
    let unit = render_unit(&PathBuf::from("/usr/local/bin/beagle-agentd"), "/usr/bin");
    assert!(unit.contains("ExecStart=/usr/local/bin/beagle-agentd"));
    assert!(unit.contains("Restart=always"));
    // Starts on login.
    assert!(unit.contains("WantedBy=default.target"));
    assert!(unit.starts_with("[Unit]"));
    assert!(unit.contains("[Service]"));
    assert!(unit.contains("[Install]"));
}

#[test]
fn plist_sets_the_daemon_path_so_it_finds_gh_and_claude() {
    let plist = render_plist(&LaunchdConfig {
        label: "com.beagle.agentd".to_string(),
        program: PathBuf::from("/opt/homebrew/bin/beagle-agentd"),
        log_dir: PathBuf::from("/tmp/logs"),
        path: "/opt/homebrew/bin:/Users/me/.local/bin:/usr/bin:/bin".to_string(),
    });
    assert!(
        plist.contains(
            "<key>EnvironmentVariables</key>\n    <dict>\n        <key>PATH</key>\n        \
             <string>/opt/homebrew/bin:/Users/me/.local/bin:/usr/bin:/bin</string>\n    </dict>"
        ),
        "{plist}"
    );
}

#[test]
fn plist_escapes_xml_in_paths() {
    let plist = render_plist(&LaunchdConfig {
        label: "com.beagle.agentd".to_string(),
        program: PathBuf::from("/Users/a&b/bin/beagle-agentd"),
        log_dir: PathBuf::from("/tmp/logs"),
        path: "/x<y>:/bin".to_string(),
    });
    assert!(plist.contains("<string>/Users/a&amp;b/bin/beagle-agentd</string>"));
    assert!(plist.contains("<string>/x&lt;y&gt;:/bin</string>"));
}

#[test]
fn unit_sets_a_quoted_path_environment() {
    let unit = render_unit(
        &PathBuf::from("/usr/local/bin/beagle-agentd"),
        "/opt/homebrew/bin:/home/me/100%/bin:/usr/bin",
    );
    assert!(
        unit.contains("Environment=\"PATH=/opt/homebrew/bin:/home/me/100%%/bin:/usr/bin\"\n"),
        "{unit}"
    );
}

#[test]
fn parse_and_format_status_summarizes_agents() {
    let line = r#"{"type":"status","status":{"version":"0.1.0","agents":[
        {"id":"rca","enabled":true,"running":true,"last_tick":"1723550400","active_sessions":1,"last_results":["x -> published"]},
        {"id":"other","enabled":true,"running":false,"last_tick":null,"active_sessions":0,"last_results":[]}
    ]}}"#;
    let snapshot = parse_status(line).expect("parse");
    assert_eq!(snapshot.version, "0.1.0");
    assert_eq!(snapshot.agents.len(), 2);
    assert!(snapshot.agents[0].running);
    assert_eq!(snapshot.agents[0].active_sessions, 1);
    assert_eq!(snapshot.agents[1].last_tick, None);

    let out = format_snapshot(&snapshot);
    assert!(out.contains("beagle-agentd 0.1.0"));
    assert!(out.contains("rca: running (last tick 1723550400)"), "{out}");
    assert!(out.contains("other: paused"), "{out}");
}

#[test]
fn parse_status_handles_no_agents() {
    let snapshot = parse_status(r#"{"type":"status","status":{"version":"0.1.0","agents":[]}}"#)
        .expect("parse");
    assert!(format_snapshot(&snapshot).contains("(no agents)"));
}

#[test]
fn newest_log_picks_the_latest_dot_log() {
    let dir = tempfile::tempdir().expect("tempdir");
    // Empty dir → nothing.
    assert!(newest_log_in(dir.path()).is_none());

    std::fs::write(dir.path().join("old.log"), "a").expect("write old");
    std::thread::sleep(std::time::Duration::from_millis(20));
    std::fs::write(dir.path().join("new.log"), "b").expect("write new");
    std::fs::write(dir.path().join("notes.txt"), "c").expect("write txt");

    let newest = newest_log_in(dir.path()).expect("a log");
    assert_eq!(newest.file_name().expect("name"), "new.log");
}

#[test]
fn newest_log_on_missing_dir_is_none() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert!(newest_log_in(&dir.path().join("nope")).is_none());
}

#[test]
fn parse_status_rejects_garbage_and_errors() {
    assert!(parse_status("not json").is_err());
    // An error response surfaces its message.
    let err = parse_status(r#"{"type":"error","message":"boom"}"#).expect_err("error");
    assert_eq!(err, "boom");
    // An unexpected type is an error too.
    assert!(parse_status(r#"{"type":"ok"}"#).is_err());
}

#[test]
fn config_path_prefers_xdg_then_home() {
    use std::ffi::OsStr;
    let xdg = OsStr::new("/cfg");
    let home = OsStr::new("/home/me");
    assert_eq!(
        config_path_from(Some(xdg), Some(home)),
        Some(PathBuf::from("/cfg/beagle/agents.toml"))
    );
    // Empty XDG falls through to HOME.
    assert_eq!(
        config_path_from(Some(OsStr::new("")), Some(home)),
        Some(PathBuf::from("/home/me/.config/beagle/agents.toml"))
    );
    assert_eq!(
        config_path_from(None, Some(home)),
        Some(PathBuf::from("/home/me/.config/beagle/agents.toml"))
    );
    assert_eq!(config_path_from(None, None), None);
}
