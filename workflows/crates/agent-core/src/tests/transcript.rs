use super::*;

fn event(kind: &str, payload: &str) -> Event {
    Event {
        id: 1,
        run_id: "r".to_owned(),
        ts: Utc::now(),
        kind: kind.to_owned(),
        payload: Some(payload.to_owned()),
    }
}

#[test]
fn control_sequences_are_stripped_from_bodies() {
    // A carriage return snaps the cursor to column 0 and paints over the
    // frame; an ANSI colour sequence repositions/recolours it.
    assert_eq!(sanitize("do\rne"), "done");
    assert_eq!(sanitize("\u{1b}[32mok\u{1b}[0m"), "ok");
    assert_eq!(sanitize("\u{1b}[1;31merr\u{1b}[m!"), "err!");
    // OSC (window title) runs to BEL or ESC backslash.
    assert_eq!(sanitize("\u{1b}]0;title\u{7}x"), "x");
    assert_eq!(sanitize("\u{1b}]0;title\u{1b}\\x"), "x");
    // Newlines survive — the log view splits on them.
    assert_eq!(sanitize("a\nb"), "a\nb");
    // Tabs become spaces rather than vanishing.
    assert_eq!(sanitize("a\tb"), "a    b");
    // Other C0 controls and DEL go.
    assert_eq!(sanitize("a\u{8}\u{0}\u{7f}b"), "ab");
    // Ordinary text, including non-ASCII, is untouched.
    assert_eq!(sanitize("plain — ✓ 日本"), "plain — ✓ 日本");
}

#[test]
fn plain_payloads_pass_through() {
    let out = prettify(&event("state", "running"));
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].tag, "state");
    assert_eq!(out[0].body, "running");
}

#[test]
fn init_summarizes_and_noise_is_dropped() {
    let init = r#"{"type":"system","subtype":"init","cwd":"/home/m","tools":["A","B"]}"#;
    let out = prettify(&event("system", init));
    assert_eq!(out[0].tag, "init");
    assert_eq!(out[0].body, "cwd /home/m · 2 tools");

    let noise = r#"{"type":"system","subtype":"thinking_tokens","estimated_tokens":50}"#;
    assert_eq!(prettify(&event("system", noise)), [] as [PrettyEvent; 0]);
    let rate = r#"{"type":"rate_limit_event","rate_limit_info":{}}"#;
    assert_eq!(
        prettify(&event("rate_limit_event", rate)),
        [] as [PrettyEvent; 0]
    );
    let hook_ok = r#"{"type":"system","subtype":"hook_response","exit_code":0}"#;
    assert_eq!(prettify(&event("system", hook_ok)), [] as [PrettyEvent; 0]);
}

#[test]
fn assistant_blocks_expand_by_kind() {
    let msg = r#"{"type":"assistant","message":{"content":[
        {"type":"thinking","thinking":""},
        {"type":"text","text":"Hello!"},
        {"type":"tool_use","name":"Bash","input":{"command":"git status"}}
    ]}}"#;
    let out = prettify(&event("assistant", msg));
    assert_eq!(out.len(), 2); // empty thinking dropped
    assert_eq!(
        (out[0].tag.as_str(), out[0].body.as_str()),
        ("assistant", "Hello!")
    );
    assert_eq!(
        (out[1].tag.as_str(), out[1].body.as_str()),
        ("tool_use", "Bash git status")
    );
}

#[test]
fn signature_only_thinking_message_is_silent() {
    let msg = r#"{"type":"assistant","message":{"content":[
        {"type":"thinking","thinking":"","signature":"CAIS..."}
    ]}}"#;
    assert_eq!(prettify(&event("assistant", msg)), [] as [PrettyEvent; 0]);
}

#[test]
fn tool_results_and_results_render_text() {
    let user = r#"{"type":"user","message":{"content":[
        {"type":"tool_result","content":[{"type":"text","text":"3 files"}]}
    ]}}"#;
    let out = prettify(&event("user", user));
    assert_eq!(
        (out[0].tag.as_str(), out[0].body.as_str()),
        ("tool_result", "3 files")
    );

    let result =
        r#"{"type":"result","subtype":"success","duration_ms":2977,"num_turns":1,"result":"Hi"}"#;
    let out = prettify(&event("result", result));
    assert_eq!(out[0].tag, "result");
    assert_eq!(out[0].body, "success · 3.0s · 1 tool call\nHi");
}

#[test]
fn a_script_line_becomes_a_levelled_entry_with_its_source() {
    let out = prettify(&event("raw", "gather: wrote 6 files"));
    assert_eq!(out[0].tag, "info");
    assert_eq!(out[0].source.as_deref(), Some("gather"));
    assert_eq!(out[0].body, "wrote 6 files");

    let warn = prettify(&event("raw", "WARN pr-watch: gh could not read the PR"));
    assert_eq!(warn[0].tag, "warn");
}

#[test]
fn an_event_stored_cut_off_reads_as_clipped_not_as_raw_json() {
    // Kind came from the full line; the stored payload is a fragment.
    let cut = r#"{"type":"user","message":{"content":[{"type":"tool_result","content":"half a fi"#;
    let out = prettify(&event("user", cut));
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].tag, "clipped");
    assert!(!out[0].body.contains('{'), "no JSON leaks into the view");
}

#[test]
fn what_a_run_was_refused_gets_a_line_each() {
    let result = r#"{"type":"result","subtype":"success","num_turns":63,"total_cost_usd":1.43,
        "permission_denials":[{"tool_name":"Bash","tool_input":{"command":"git fetch origin main"}}]}"#;
    let out = prettify(&event("result", result));
    assert!(out[0].body.contains("63 tool calls"));
    assert!(out[0].body.contains("$1.43"));
    assert_eq!(out[1].tag, "denied");
    assert_eq!(out[1].body, "Bash git fetch origin main");
}

#[test]
fn codex_items_map_to_shared_tags() {
    let noise = r#"{"thread_id":"x","type":"thread.started"}"#;
    assert_eq!(
        prettify(&event("thread.started", noise)),
        [] as [PrettyEvent; 0]
    );
    assert_eq!(
        prettify(&event("turn.started", r#"{"type":"turn.started"}"#)),
        [] as [PrettyEvent; 0]
    );

    let msg = r#"{"type":"item.completed","item":{"id":"item_0","type":"agent_message","text":"Reading it now."}}"#;
    let out = prettify(&event("item.completed", msg));
    assert_eq!(
        (out[0].tag.as_str(), out[0].body.as_str()),
        ("assistant", "Reading it now.")
    );

    let cmd = r#"{"type":"item.completed","item":{"type":"command_execution",
        "command":"cat finding.txt","aggregated_output":"2% used\n","exit_code":0}}"#;
    let out = prettify(&event("item.completed", cmd));
    assert_eq!(
        (out[0].tag.as_str(), out[0].body.as_str()),
        ("tool_use", "cat finding.txt")
    );
    assert_eq!(
        (out[1].tag.as_str(), out[1].body.as_str()),
        ("tool_result", "2% used")
    );

    let failing = r#"{"type":"item.completed","item":{"type":"command_execution",
        "command":"false","aggregated_output":"","exit_code":1}}"#;
    let out = prettify(&event("item.completed", failing));
    assert_eq!(out[1].body, "exit 1 · ");

    let turn = r#"{"type":"turn.completed","usage":{"input_tokens":30977,"output_tokens":131}}"#;
    let out = prettify(&event("turn.completed", turn));
    assert_eq!(out[0].tag, "result");
    assert_eq!(out[0].body, "turn completed · 30977 in / 131 out tokens");
}

#[test]
fn plain_text_formats_multiline_bodies() {
    let text = plain_text(&[event("state", "running")]);
    assert!(text.contains("state"));
    assert!(text.contains("running"));
}
