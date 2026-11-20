//! Session recording tests: hook payloads, appends, listing and picking.

// Tests fail loudly on setup errors; unwrap/expect/panic is the clearest way.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::listing::*;
use super::*;
use serde_json::json;

fn now() -> OffsetDateTime {
    time::macros::datetime!(2026-11-12 15:00 UTC)
}

fn here() -> Location {
    Location {
        tab: Some("t1".into()),
        pane: Some("p1".into()),
    }
}

fn pre(session: &str, id: &str, command: &str) -> Value {
    json!({
        "session_id": session, "hook_event_name": "PreToolUse", "tool_name": "Bash",
        "tool_use_id": id, "cwd": "/repo",
        "tool_input": {"command": command, "description": "Run tests"}
    })
}

#[test]
fn pre_tool_use_becomes_start() {
    let (session, event) =
        event_from_hook(&pre("s1", "t-1", "cargo test"), &here(), now()).unwrap();
    assert_eq!(session, "s1");
    assert_eq!(
        event,
        Event::Start {
            ts: now(),
            id: "t-1".into(),
            command: "cargo test".into(),
            description: Some("Run tests".into()),
            cwd: Some("/repo".into()),
            agent: None,
            background: false,
            tab: Some("t1".into()),
            pane: Some("p1".into()),
            transcript: None,
        }
    );
}

#[test]
fn post_and_failure_become_end_and_fail() {
    let post = json!({
        "session_id": "s1", "hook_event_name": "PostToolUse", "tool_name": "Bash",
        "tool_use_id": "t-1", "duration_ms": 1200,
        "tool_response": {"stdout": "ok", "stderr": "", "interrupted": false}
    });
    let (_, end) = event_from_hook(&post, &here(), now()).unwrap();
    assert!(
        matches!(end, Event::End { ref stdout, duration_ms: Some(1200), .. } if stdout == "ok")
    );
    let fail = json!({
        "session_id": "s1", "hook_event_name": "PostToolUseFailure", "tool_name": "Bash",
        "tool_use_id": "t-2", "error": "Exit code 1\nboom", "is_interrupt": false
    });
    let (_, fail) = event_from_hook(&fail, &here(), now()).unwrap();
    assert!(matches!(fail, Event::Fail { ref error, .. } if error.starts_with("Exit code 1")));
}

#[test]
fn other_tools_and_bad_sessions_are_ignored() {
    let mut edit = pre("s1", "t", "x");
    edit["tool_name"] = json!("Edit");
    assert!(event_from_hook(&edit, &here(), now()).is_none());
    assert!(event_from_hook(&pre("../etc", "t", "x"), &here(), now()).is_none());
    assert!(event_from_hook(&json!("garbage"), &here(), now()).is_none());
}

#[test]
fn output_is_capped_on_a_char_boundary() {
    let long = "é".repeat(MAX_OUTPUT_BYTES);
    let capped = cap(long);
    assert!(capped.len() < MAX_OUTPUT_BYTES + 64);
    assert!(capped.ends_with("[scout: output truncated]"));
}

#[test]
fn append_round_trips_and_lists() {
    let root = tempfile::tempdir().unwrap();
    let (session, start) = event_from_hook(&pre("s1", "t-1", "ls"), &here(), now()).unwrap();
    append(root.path(), &session, &start).unwrap();
    let text = fs::read_to_string(file(root.path(), "s1")).unwrap();
    assert_eq!(parse_line(text.lines().next().unwrap()), Some(start));
    let sessions = list(root.path());
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].last_command.as_deref(), Some("ls"));
    assert_eq!(sessions[0].tab.as_deref(), Some("t1"));
    assert_eq!(sessions[0].commands, 1);
}

#[test]
fn describes_from_the_transcript_tail() {
    let text = [
        r#"{"type":"ai-title","aiTitle":"Old title"}"#,
        r#"{"type":"user","message":"hi"}"#,
        r#"{"type":"last-prompt","lastPrompt":"fix the\nflaky test"}"#,
        r#"{"type":"ai-title","aiTitle":"Fix flaky uploader test"}"#,
        "not json",
    ]
    .join("\n");
    assert_eq!(
        describe_transcript(&text),
        Description {
            title: Some("Fix flaky uploader test".into()),
            last_prompt: Some("fix the flaky test".into()),
        }
    );
    assert_eq!(describe_transcript(""), Description::default());
    let root = tempfile::tempdir().unwrap();
    let transcript = root.path().join("t.jsonl");
    fs::write(&transcript, &text).unwrap();
    let mut payload = pre("s9", "t", "ls");
    payload["transcript_path"] = serde_json::json!(transcript);
    let (_, start) = event_from_hook(&payload, &here(), now()).unwrap();
    append(root.path(), "s9", &start).unwrap();
    let info = &list(root.path())[0];
    assert_eq!(
        describe(info).title.as_deref(),
        Some("Fix flaky uploader test")
    );
}

#[test]
fn resolve_expands_unique_prefixes() {
    let root = tempfile::tempdir().unwrap();
    for id in ["4f85aa", "4f85bb", "9c01"] {
        let (_, start) = event_from_hook(&pre(id, "t", "ls"), &here(), now()).unwrap();
        append(root.path(), id, &start).unwrap();
    }
    assert_eq!(resolve(root.path(), "9c").unwrap(), "9c01");
    assert_eq!(resolve(root.path(), "4f85bb").unwrap(), "4f85bb");
    assert_eq!(resolve(root.path(), "new").unwrap(), "new");
    assert_eq!(resolve(root.path(), "4f85").unwrap_err().len(), 2);
}

#[test]
fn pick_prefers_tab_then_cwd_then_newest() {
    let s = |id: &str, tab: Option<&str>, cwd: &str| SessionInfo {
        id: id.into(),
        path: PathBuf::new(),
        modified: SystemTime::UNIX_EPOCH,
        tab: tab.map(str::to_owned),
        cwd: Some(cwd.into()),
        last_command: None,
        transcript: None,
        commands: 1,
    };
    let sessions = [
        s("newest", None, "/a"),
        s("in-dir", None, "/b"),
        s("in-tab", Some("t9"), "/c"),
    ];
    assert_eq!(
        pick(&sessions, Some("t9"), Path::new("/b")).unwrap().id,
        "in-tab"
    );
    assert_eq!(
        pick(&sessions, Some("t0"), Path::new("/b")).unwrap().id,
        "in-dir"
    );
    assert_eq!(pick(&sessions, None, Path::new("/z")).unwrap().id, "newest");
    assert!(pick(&[], None, Path::new("/")).is_none());
}

#[test]
fn recording_into_a_fresh_git_backed_store_ignores_sessions() {
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir(repo.path().join(".git")).unwrap();
    let store = repo.path().join("notes/scout");
    let (_, start) = event_from_hook(&pre("s1", "t", "ls"), &here(), now()).unwrap();
    append(&store, "s1", &start).unwrap();
    let ignore = fs::read_to_string(store.join(".gitignore")).unwrap();
    for line in crate::gitignore::LINES {
        assert!(ignore.lines().any(|l| l == line), "{line} in {ignore}");
    }
    // Later events in the same recording leave the file alone.
    fs::write(store.join(".gitignore"), "custom\n").unwrap();
    append(&store, "s1", &start).unwrap();
    assert_eq!(
        fs::read_to_string(store.join(".gitignore")).unwrap(),
        "custom\n"
    );
}

#[test]
fn recording_outside_git_writes_no_gitignore() {
    let root = tempfile::tempdir().unwrap();
    let (_, start) = event_from_hook(&pre("s2", "t", "ls"), &here(), now()).unwrap();
    append(root.path(), "s2", &start).unwrap();
    assert!(!root.path().join(".gitignore").exists());
}
