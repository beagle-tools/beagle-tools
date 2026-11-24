//! Tests for agent specs: the auth modes that were removed still load.
#![allow(clippy::unwrap_used)]

use super::{AgentSpec, AuthMode};

fn spec_with_auth(auth: &str) -> String {
    format!(
        r#"{{"name":"a","runner":{{"kind":"claude_headless"}},"auth":{auth},
            "model":null,"append_system_prompt":null,"prompt":"p"}}"#
    )
}

#[test]
fn cli_session_round_trips() {
    let spec: AgentSpec =
        serde_json::from_str(&spec_with_auth(r#"{"kind":"cli_session"}"#)).unwrap();
    assert_eq!(spec.auth, AuthMode::CliSession);
    let json = serde_json::to_value(&spec).unwrap();
    assert_eq!(json["auth"], serde_json::json!({ "kind": "cli_session" }));
}

#[test]
fn removed_auth_modes_load_as_cli_login() {
    for old in [
        r#"{"kind":"api_key","env_var":"ANTHROPIC_API_KEY"}"#,
        r#"{"kind":"endpoint","base_url":"https://llm.example.com","env_var":null}"#,
        r#"{"kind":"endpoint","base_url":"https://llm.example.com","env_var":"KEY"}"#,
    ] {
        let spec: AgentSpec = serde_json::from_str(&spec_with_auth(old)).unwrap();
        assert_eq!(spec.auth, AuthMode::CliSession, "{old}");
    }
}
