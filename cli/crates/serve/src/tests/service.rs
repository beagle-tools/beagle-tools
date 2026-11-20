// Tests fail loudly on setup errors; unwrap/expect is the clearest way.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::*;

fn plist() -> Plist {
    Plist {
        program: PathBuf::from("/opt/homebrew/bin/beagle"),
        log_dir: PathBuf::from("/Users/octocat/.local/state/beagle/serve"),
        working_dir: PathBuf::from("/Users/octocat"),
        env: vec![
            ("PATH".into(), "/opt/homebrew/bin:/usr/bin:/bin".into()),
            ("SCOUT_ROOT".into(), "/Users/octocat/scout & notes".into()),
        ],
    }
}

#[test]
fn the_plist_runs_beagle_serve_at_login_and_keeps_it_alive() {
    let text = render_plist(&plist());
    assert!(text.contains("<string>com.beagle.serve</string>"));
    assert!(text.contains(
        "<array>\n        <string>/opt/homebrew/bin/beagle</string>\n        <string>serve</string>\n    </array>"
    ));
    assert!(text.contains("<key>RunAtLoad</key>\n    <true/>"));
    assert!(text.contains("<key>KeepAlive</key>\n    <true/>"));
    assert!(text.contains("<key>ThrottleInterval</key>\n    <integer>30</integer>"));
    assert!(text.contains("<key>WorkingDirectory</key>\n    <string>/Users/octocat</string>"));
    assert!(text.contains("/Users/octocat/.local/state/beagle/serve/serve.err.log"));
    assert!(
        text.contains("<key>PATH</key>\n        <string>/opt/homebrew/bin:/usr/bin:/bin</string>")
    );
}

#[test]
fn plist_values_are_xml_escaped() {
    let text = render_plist(&plist());
    assert!(text.contains("<string>/Users/octocat/scout &amp; notes</string>"));
    assert!(!text.contains("scout & notes"));
}

#[test]
fn the_plist_is_well_formed_enough_for_launchd() {
    let text = render_plist(&plist());
    assert!(text.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist"));
    for tag in ["dict", "array", "plist"] {
        assert_eq!(
            text.matches(&format!("<{tag}")).count(),
            text.matches(&format!("</{tag}>")).count(),
            "{tag}"
        );
    }
}

#[test]
fn launchctl_print_yields_state_and_pid() {
    let out = "gui/501/com.beagle.serve = {\n\tactive count = 1\n\tpath = /Users/octocat/Library/LaunchAgents/com.beagle.serve.plist\n\tstate = running\n\tprogram = /opt/homebrew/bin/beagle\n\tpid = 4242\n}\n";
    assert_eq!(parse_print(out), (Some("running".into()), Some(4242)));
    let waiting = "\tstate = waiting\n\tlast exit code = 1\n";
    assert_eq!(parse_print(waiting), (Some("waiting".into()), None));
}

#[test]
fn status_lines_say_what_to_do() {
    assert!(Status::NotInstalled
        .describe()
        .contains("beagle serve install"));
    let running = Status::Loaded {
        plist: PathBuf::from("/p"),
        state: "running".into(),
        pid: Some(7),
    };
    assert!(running.is_running());
    assert_eq!(running.describe(), "beagle serve: running (pid 7)");
    let stopped = Status::Loaded {
        plist: PathBuf::from("/p"),
        state: "waiting".into(),
        pid: None,
    };
    assert!(!stopped.is_running());
    assert!(stopped.describe().contains("serve.err.log"));
}

/// macOS's own plist parser accepts the file launchd will load.
#[cfg(target_os = "macos")]
#[test]
fn plutil_accepts_the_plist() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("com.beagle.serve.plist");
    std::fs::write(&path, render_plist(&plist())).unwrap();
    let out = output_with_timeout(
        Command::new("/usr/bin/plutil").arg("-lint").arg(&path),
        TOOL_TIMEOUT,
    )
    .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
}
