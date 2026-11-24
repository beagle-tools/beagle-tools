//! Transcript ETL tests: parsing, task slicing and incremental sync.

// Tests fail loudly on setup errors; unwrap/expect/panic is the clearest way.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::parse::*;
use super::sync::*;
use super::*;

const PROMPT: &str = r#"{"type":"user","timestamp":"2026-11-16T15:00:00Z","origin":{"kind":"human"},"message":{"role":"user","content":"fix the flaky test"}}"#;
const SLASH: &str = r#"{"type":"user","timestamp":"t","origin":{"kind":"human"},"message":{"content":"<command-message>scout-build</command-message>\n<command-name>/scout-build</command-name>\n<command-args>demo</command-args>"}}"#;
const META: &str = r#"{"type":"user","timestamp":"t","isMeta":true,"message":{"content":"Base directory for this skill"}}"#;
const ASSISTANT: &str = r#"{"type":"assistant","timestamp":"t","thinkingDurationMs":1200,"message":{"content":[{"type":"thinking","thinking":"","signature":"x"},{"type":"text","text":"Running the tests."},{"type":"tool_use","id":"toolu_1","name":"Bash","input":{"command":"cargo test","description":"Run the tests"}},{"type":"tool_use","id":"toolu_2","name":"Edit","input":{"file_path":"/r/src/a.rs","old_string":"a","new_string":"b"}}]}}"#;
const RESULT: &str = r#"{"type":"user","timestamp":"t","message":{"content":[{"type":"tool_result","tool_use_id":"toolu_1","content":"test result: ok","is_error":false},{"type":"tool_result","tool_use_id":"toolu_3","content":[{"type":"text","text":"Exit code 1"}],"is_error":true}]},"toolUseResult":{"stdout":"x"}}"#;
const ANSWER: &str = r#"{"type":"user","timestamp":"t","message":{"content":[{"type":"tool_result","tool_use_id":"toolu_9","content":"User answered"}]},"toolUseResult":{"answers":{"Push?":"Push branch only"}}}"#;

#[test]
fn prompts_and_slash_commands() {
    assert_eq!(
        parse_line(PROMPT),
        vec![Entry::Prompt {
            ts: "2026-11-16T15:00:00Z".into(),
            text: "fix the flaky test".into(),
            cwd: None,
        }]
    );
    assert_eq!(
        parse_line(SLASH),
        vec![Entry::Prompt {
            ts: "t".into(),
            text: "/scout-build demo".into(),
            cwd: None,
        }]
    );
    assert_eq!(parse_line(META).len(), 0);
    assert_eq!(parse_line("not json").len(), 0);
    assert_eq!(parse_line(r#"{"type":"api-request"}"#).len(), 0);
}

#[test]
fn assistant_blocks() {
    let entries = parse_line(ASSISTANT);
    assert_eq!(entries.len(), 4);
    assert_eq!(
        entries[0],
        Entry::Thinking {
            ts: "t".into(),
            text: None,
            ms: Some(1200)
        }
    );
    assert_eq!(
        entries[1],
        Entry::Text {
            ts: "t".into(),
            text: "Running the tests.".into()
        }
    );
    assert_eq!(
        entries[2],
        Entry::Tool {
            ts: "t".into(),
            id: "toolu_1".into(),
            name: "Bash".into(),
            summary: "cargo test".into(),
            detail: Some("Run the tests".into()),
        }
    );
    assert!(
        matches!(&entries[3], Entry::Tool { name, summary, .. } if name == "Edit" && summary == "/r/src/a.rs")
    );
}

#[test]
fn results_errors_and_answers() {
    let entries = parse_line(RESULT);
    assert_eq!(entries.len(), 2);
    assert!(
        matches!(&entries[0], Entry::Result { text, error: false, .. } if text == "test result: ok")
    );
    assert!(
        matches!(&entries[1], Entry::Result { text, error: true, .. } if text == "Exit code 1")
    );
    assert!(
        matches!(&parse_line(ANSWER)[0], Entry::Result { text, .. } if text == "Push? → Push branch only")
    );
}

#[test]
fn edits_and_writes_become_changes() {
    let edit = r#"{"type":"user","timestamp":"t","message":{"content":[{"type":"tool_result","tool_use_id":"e1","content":"ok"}]},"toolUseResult":{"filePath":"/r/src/a.rs","structuredPatch":[{"lines":[" x","-old","+new","+more"]}]}}"#;
    let create = r#"{"type":"user","timestamp":"t","message":{"content":[{"type":"tool_result","tool_use_id":"w1","content":"ok"}]},"toolUseResult":{"type":"create","filePath":"/r/new.sh","content":"a\nb\nc\n"}}"#;
    assert_eq!(
        parse_line(edit)[1],
        Entry::Change {
            ts: "t".into(),
            id: "e1".into(),
            path: "/r/src/a.rs".into(),
            created: false,
            added: 2,
            removed: 1
        }
    );
    assert_eq!(
        parse_line(create)[1],
        Entry::Change {
            ts: "t".into(),
            id: "w1".into(),
            path: "/r/new.sh".into(),
            created: true,
            added: 3,
            removed: 0
        }
    );
}

#[test]
fn etl_writes_the_view_once_per_change() {
    let dir = tempfile::tempdir().unwrap();
    let store = crate::store::Store::new(dir.path());
    let slug = |s| crate::model::Slug::parse(s).unwrap();
    store.create_project(&slug("p"), "P", vec![]).unwrap();
    store.add_task("p", &slug("a"), "A", vec![]).unwrap();
    store.attach_session("p", "1", "s").unwrap();
    let transcript = dir.path().join("t.jsonl");
    let project = dir.path().join("p");
    fs::write(&transcript, format!("{PROMPT}\n")).unwrap();
    assert!(etl(&transcript, &project, "s", &store).unwrap());
    let view = fs::read_to_string(view_file(&cache_file(&project, "s"), "01-a")).unwrap();
    assert!(view.contains("fix the flaky test"), "{view}");
    assert!(
        !etl(&transcript, &project, "s", &store).unwrap(),
        "nothing new"
    );
    let held = Lock::take(&cache_file(&project, "s").with_extension("lock")).unwrap();
    let mut f = OpenOptions::new().append(true).open(&transcript).unwrap();
    writeln!(f, "{ASSISTANT}").unwrap();
    assert!(
        !etl(&transcript, &project, "s", &store).unwrap(),
        "locked: skipped"
    );
    drop(held);
    assert!(etl(&transcript, &project, "s", &store).unwrap());
}

#[test]
fn each_task_sees_its_own_part_of_the_session() {
    let dir = tempfile::tempdir().unwrap();
    let store = crate::store::Store::new(dir.path());
    let slug = |s| crate::model::Slug::parse(s).unwrap();
    store.create_project(&slug("p"), "P", vec![]).unwrap();
    store.add_task("p", &slug("one"), "One", vec![]).unwrap();
    store.add_task("p", &slug("two"), "Two", vec![]).unwrap();
    store
        .add_task("p", &slug("three"), "Three", vec![])
        .unwrap();
    let project = store.project("p").unwrap();
    let prompt = |t: &str| Entry::Prompt {
        ts: "t".into(),
        text: t.into(),
        cwd: None,
    };
    let bash = |c: &str| Entry::Tool {
        ts: "t".into(),
        id: c.into(),
        name: "Bash".into(),
        summary: c.into(),
        detail: None,
    };
    let result = |id: &str| Entry::Result {
        ts: "t".into(),
        id: id.into(),
        text: "ok".into(),
        error: false,
    };
    let entries = vec![
        prompt("unrelated earlier work"),           // 0
        bash("ls"),                                 // 1
        prompt("do task one"),                      // 2
        bash("P=p; scout task start $P 1"),         // 3
        bash("cargo test"),                         // 4
        bash("scout task status p 1 todo"),         // 5: paused
        bash("scout task start p two"),             // 6
        bash("cargo build"),                        // 7
        bash("scout log p --task 1 back-on-one"),   // 8: one again
        result("scout log p --task 1 back-on-one"), // 9
        Entry::Text {
            ts: "t".into(),
            text: "done".into(),
        }, // 10
        bash("ls again"),                           // 11
    ];
    let one = task_slice(&entries, &project, &project.tasks[0]);
    assert_eq!(one, &entries[2..11], "first start .. last mention + output");
    let two = task_slice(&entries, &project, &project.tasks[1]);
    assert_eq!(two, &entries[2..7], "not working: up to its last mention");
    let three = task_slice(&entries, &project, &project.tasks[2]);
    assert_eq!(three, &entries[..], "no marker: the whole session");
    store
        .set_task_status("p", "two", crate::model::TaskStatus::Working)
        .unwrap();
    let project = store.project("p").unwrap();
    let two = task_slice(&entries, &project, &project.tasks[1]);
    assert_eq!(two, &entries[2..], "working: to the end");
}

#[test]
fn old_format_copies_are_rebuilt() {
    let dir = tempfile::tempdir().unwrap();
    let transcript = dir.path().join("t.jsonl");
    let cache = dir.path().join("s.jsonl");
    fs::write(&transcript, format!("{PROMPT}\n")).unwrap();
    fs::write(
        &cache,
        "{\"kind\":\"notice\",\"ts\":\"t\",\"text\":\"old\"}\n",
    )
    .unwrap();
    fs::write(cursor_file(&cache), "9999").unwrap();
    assert_eq!(sync(&transcript, &cache).unwrap(), 1);
    assert_eq!(read_cache(&cache).len(), 1, "old entries dropped");
}

#[test]
fn sync_is_incremental_and_waits_for_whole_lines() {
    let dir = tempfile::tempdir().unwrap();
    let transcript = dir.path().join("t.jsonl");
    let cache = dir.path().join("p/sessions/s.jsonl");
    fs::write(&transcript, format!("{PROMPT}\n{}", &ASSISTANT[..20])).unwrap();
    assert_eq!(sync(&transcript, &cache).unwrap(), 1);
    assert_eq!(sync(&transcript, &cache).unwrap(), 0, "nothing new");
    let mut f = OpenOptions::new().append(true).open(&transcript).unwrap();
    writeln!(f, "{}", &ASSISTANT[20..]).unwrap();
    assert_eq!(sync(&transcript, &cache).unwrap(), 4);
    assert_eq!(read_cache(&cache).len(), 5);
    fs::write(&transcript, format!("{PROMPT}\n")).unwrap();
    assert_eq!(sync(&transcript, &cache).unwrap(), 1, "shrunk: start over");
    assert_eq!(read_cache(&cache).len(), 1);
}

#[test]
fn finds_transcripts_and_rejects_bad_ids() {
    let home = tempfile::tempdir().unwrap();
    let dir = home.path().join(".claude/projects/-Users-me-repo");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("abc-123.jsonl"), "").unwrap();
    assert_eq!(
        find_transcript(home.path(), "abc-123"),
        Some(dir.join("abc-123.jsonl"))
    );
    assert_eq!(find_transcript(home.path(), "nope"), None);
    assert_eq!(find_transcript(home.path(), "../etc"), None);
}

#[test]
fn caps_on_char_boundaries() {
    assert_eq!(cap("ééé", 3), "é…");
    assert_eq!(cap("abc", 5), "abc");
}
