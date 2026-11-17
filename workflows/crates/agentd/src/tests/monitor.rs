use super::*;

#[test]
fn exit_marker_parsing() {
    assert_eq!(parse_exit_marker("__AGENTD_EXIT__:0\n"), Some(0));
    assert_eq!(parse_exit_marker("noise\n__AGENTD_EXIT__:17\n"), Some(17));
    // The last marker wins.
    assert_eq!(
        parse_exit_marker("__AGENTD_EXIT__:1\n__AGENTD_EXIT__:0\n"),
        Some(0)
    );
    assert_eq!(parse_exit_marker("no marker here\n"), None);
    assert_eq!(parse_exit_marker("__AGENTD_EXIT__:not-a-number\n"), None);
    assert_eq!(parse_exit_marker(""), None);
}

#[test]
fn final_state_from_exit_code() {
    assert_eq!(final_state(Some(0)), RunState::Succeeded);
    assert_eq!(final_state(Some(2)), RunState::Failed);
    assert_eq!(final_state(None), RunState::Failed);
}

#[test]
fn classify_json_lines_by_type_field() {
    let (kind, payload) = classify_line(r#"{"type":"tool_use","name":"Bash"}"#).unwrap();
    assert_eq!(kind, "tool_use");
    assert_eq!(payload, r#"{"type":"tool_use","name":"Bash"}"#);
}

#[test]
fn classify_non_json_and_typeless_json_as_raw() {
    assert_eq!(classify_line("plain text output").unwrap().0, "raw");
    assert_eq!(classify_line(r#"{"no_type":true}"#).unwrap().0, "raw");
    assert_eq!(classify_line(r#"["an","array"]"#).unwrap().0, "raw");
    assert_eq!(classify_line(r#"{"type":42}"#).unwrap().0, "raw");
}

#[test]
fn classify_skips_marker_and_blank_lines() {
    assert_eq!(classify_line("__AGENTD_EXIT__:0"), None);
    assert_eq!(classify_line(""), None);
    assert_eq!(classify_line("\r"), None);
}

#[test]
fn classify_keeps_long_json_payloads_valid() {
    // This test used to assert the payload was cut to exactly the cap
    // — i.e. that a long JSON line was stored as broken JSON. What the
    // views need is the opposite: under the cap AND still parseable.
    let long = format!(r#"{{"type":"message","body":"{}"}}"#, "x".repeat(8000));
    let (kind, payload) = classify_line(&long).unwrap();
    assert_eq!(kind, "message");
    assert!(payload.len() <= crate::payload::MAX_PAYLOAD_BYTES);
    let json: serde_json::Value = serde_json::from_str(&payload).unwrap();
    assert_eq!(json["type"], "message");
}

#[test]
fn complete_lines_keeps_trailing_partial() {
    let (lines, consumed) = complete_lines(b"one\ntwo\npart");
    assert_eq!(lines, vec!["one".to_owned(), "two".to_owned()]);
    assert_eq!(consumed, 8);

    let (lines, consumed) = complete_lines(b"no newline");
    assert_eq!(lines, [] as [std::string::String; 0]);
    assert_eq!(consumed, 0);

    let (lines, consumed) = complete_lines(b"done\n");
    assert_eq!(lines, vec!["done".to_owned()]);
    assert_eq!(consumed, 5);
}

#[test]
fn a_restart_resumes_after_the_lines_already_ingested() {
    // Two lines ingested before the restart; the result line was written
    // while the daemon was down and must still be read.
    let log = b"{\"type\":\"system\"}\n{\"type\":\"assistant\"}\n{\"type\":\"result\",\"total_cost_usd\":1.9}\n";
    let offset = resume_offset(log, 2);
    let rest = std::str::from_utf8(&log[usize::try_from(offset).unwrap()..]).unwrap();
    assert_eq!(rest, "{\"type\":\"result\",\"total_cost_usd\":1.9}\n");
}

#[test]
fn lines_that_make_no_event_are_not_counted() {
    // Blank lines are skipped by ingestion, so they must not use up the
    // count either.
    let log = b"{\"type\":\"system\"}\n\n{\"type\":\"assistant\"}\nnext\n";
    let offset = usize::try_from(resume_offset(log, 2)).unwrap();
    assert_eq!(&log[offset..], b"next\n");
}

#[test]
fn more_events_than_lines_never_rereads() {
    let log = b"{\"type\":\"system\"}\npartial";
    assert_eq!(resume_offset(log, 5), 18);
}
