// Tests fail loudly on setup errors; unwrap/expect is the clearest way.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::tests_support::{p8, MemoryStore};
use super::*;
use crate::push::source::{ENV_KEY, ENV_KEY_ID, ENV_TEAM_ID};

const KEY_ID: &str = "KEY1234567";
const TEAM_ID: &str = "ABC123DEFG";

fn json_output(pem: &str) -> String {
    serde_json::json!({
        ENV_KEY: pem,
        ENV_KEY_ID: KEY_ID,
        ENV_TEAM_ID: TEAM_ID,
        "UNRELATED_SECRET": "do-not-store",
        "DOPPLER_PROJECT": "example",
    })
    .to_string()
}

#[test]
fn json_objects_parse_with_multi_line_values() {
    let pem = p8();
    let parsed = parse::parse(&json_output(&pem)).unwrap();
    assert_eq!(parsed[ENV_KEY], pem);
    assert_eq!(parsed[ENV_KEY_ID], KEY_ID);
}

#[test]
fn dotenv_handles_quotes_export_comments_and_multi_line_values() {
    let text = "# a comment\n\
        export PLAIN=value one \n\
        SINGLE='it''s'\n\
        DOUBLE=\"a \\\"quoted\\\" \\n line\"\n\
        MULTI=\"-----BEGIN-----\n\
        abc\n\
        -----END-----\"\n\
        \n\
        EMPTY=\n";
    let parsed = parse::parse(text).unwrap();
    assert_eq!(parsed["PLAIN"], "value one");
    assert_eq!(parsed["SINGLE"], "it");
    assert_eq!(parsed["DOUBLE"], "a \"quoted\" \n line");
    assert_eq!(parsed["MULTI"], "-----BEGIN-----\nabc\n-----END-----");
    assert_eq!(parsed["EMPTY"], "");
}

#[test]
fn malformed_output_errors_without_echoing_values() {
    let why = parse::parse("GOOD=1\nthis line has TOPSECRET and no equals\n").unwrap_err();
    assert!(
        why.contains("line 2") && !why.contains("TOPSECRET"),
        "{why}"
    );
    let why = parse::parse("KEY=\"TOPSECRET never closed\n").unwrap_err();
    assert!(why.contains("KEY") && !why.contains("TOPSECRET"), "{why}");
    let why = parse::parse("{\"KEY\": \"TOPSECRET\",, }").unwrap_err();
    assert!(!why.contains("TOPSECRET"), "{why}");
}

#[test]
fn sync_stores_only_the_three_names() {
    let store = MemoryStore::default();
    let stored = sync_output(&store, &json_output(&p8())).unwrap();
    assert_eq!(stored, ENV_VARS.to_vec());
    let keys: Vec<String> = store.0.borrow().keys().cloned().collect();
    assert_eq!(keys.len(), 3);
    assert!(!keys.iter().any(|k| k == "UNRELATED_SECRET"));
}

#[test]
fn sync_of_a_bad_key_stores_nothing() {
    let store = MemoryStore::default();
    let output = json_output("-----BEGIN PRIVATE KEY-----\nbroken\n-----END PRIVATE KEY-----");
    assert!(sync_output(&store, &output).is_err());
    assert!(store.0.borrow().is_empty());
}

#[test]
fn sync_without_any_name_says_so() {
    let why = sync_output(&MemoryStore::default(), "OTHER=1\n").unwrap_err();
    assert!(why.contains("none of"), "{why}");
}

#[test]
fn present_and_clear_report_names_never_values() {
    let store = MemoryStore::full(&p8(), KEY_ID, TEAM_ID);
    assert!(present(&store).iter().all(|(_, p)| *p == Ok(true)));
    assert_eq!(clear(&store).unwrap(), ENV_VARS.to_vec());
    assert!(present(&store).iter().all(|(_, p)| *p == Ok(false)));
    assert_eq!(clear(&store).unwrap(), Vec::<&str>::new());
}

/// A fake secret manager: a shell script in a temp dir, run as the
/// user's command would be.
#[cfg(unix)]
#[test]
fn a_fake_command_syncs_end_to_end() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("secrets.json");
    std::fs::write(&out, json_output(&p8())).unwrap();
    let command = format!("cat '{}'", out.display());
    let stdout = run_command(&command, COMMAND_TIMEOUT).unwrap();
    let store = MemoryStore::default();
    sync_output(&store, &stdout).unwrap();
    assert_eq!(store.0.borrow()[ENV_KEY_ID], KEY_ID);
}

#[cfg(unix)]
#[test]
fn a_failing_command_reports_its_exit_and_stderr() {
    let why = run_command("echo 'not logged in' >&2; exit 3", COMMAND_TIMEOUT).unwrap_err();
    assert!(why.contains("not logged in"), "{why}");
}

#[cfg(unix)]
#[test]
fn a_hanging_command_times_out() {
    let why = run_command("sleep 5", Duration::from_millis(200)).unwrap_err();
    assert!(why.contains("timed out"), "{why}");
}
