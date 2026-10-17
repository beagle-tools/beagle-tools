//! What MCP servers the local `claude` CLI knows about, so the agent
//! editor can offer them as checkboxes instead of making you know that
//! `mcp__linear` is the magic string.
//!
//! Read-only: Tandem never adds, removes or authenticates a server —
//! that stays with `claude mcp` where the OAuth flow can be interactive.

use serde::Serialize;
use tandem_core::TandemError;

/// `claude mcp list` pings every server, which can take a while on a
/// cold network. Past this the editor is better off showing nothing
/// than hanging.
const LIST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(25);

#[derive(Debug, Clone, Serialize)]
pub struct McpServer {
    /// As `claude mcp list` prints it, e.g. "claude.ai Datadog (2)".
    pub name: String,
    /// The prefix its tools carry, e.g. "`mcp__claude_ai_Datadog_2`" —
    /// what goes in an agent's allowed-tools list.
    pub tool_prefix: String,
    /// URL or command, for telling two similar entries apart.
    pub detail: String,
    /// False when the server needs authenticating before it's usable.
    pub connected: bool,
}

/// How Claude Code derives a tool prefix from a server name: anything
/// that isn't alphanumeric becomes an underscore, and the edges are
/// trimmed. "claude.ai Datadog (2)" -> "`claude_ai_Datadog_2`".
fn tool_prefix(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut pending_underscore = false;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            if pending_underscore && !out.is_empty() {
                out.push('_');
            }
            pending_underscore = false;
            out.push(ch);
        } else {
            pending_underscore = true;
        }
    }
    format!("mcp__{out}")
}

/// One line of `claude mcp list`:
/// `name: https://host/mcp (HTTP) - ✔ Connected`
fn parse_line(line: &str) -> Option<McpServer> {
    let line = line.trim();
    // The health marker is the last " - " on the line; URLs contain
    // colons and names contain spaces, so neither is a safe split.
    let (head, status) = line.rsplit_once(" - ")?;
    let (name, detail) = head.split_once(": ")?;
    if name.is_empty() {
        return None;
    }
    Some(McpServer {
        name: name.to_owned(),
        tool_prefix: tool_prefix(name),
        detail: detail.trim().to_owned(),
        connected: status.contains("Connected"),
    })
}

/// The MCP servers an agent run would inherit. Empty (not an error)
/// when the CLI is missing or has none configured — this is a
/// convenience, never something a review depends on.
#[tauri::command]
pub async fn list_mcp_servers() -> Result<Vec<McpServer>, TandemError> {
    let mut cmd = tokio::process::Command::new("claude");
    cmd.args(["mcp", "list"])
        .env("PATH", beagle_utils::process::augmented_path())
        .stdin(std::process::Stdio::null());

    let output = match tokio::time::timeout(LIST_TIMEOUT, cmd.output()).await {
        Ok(Ok(output)) => output,
        Ok(Err(e)) => {
            tracing::warn!(error = %e, "couldn't run `claude mcp list`");
            return Ok(Vec::new());
        }
        Err(_) => {
            tracing::warn!("`claude mcp list` timed out");
            return Ok(Vec::new());
        }
    };

    let text = String::from_utf8_lossy(&output.stdout);
    Ok(text.lines().filter_map(parse_line).collect())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn derives_the_prefix_claude_code_uses() {
        assert_eq!(tool_prefix("linear"), "mcp__linear");
        assert_eq!(
            tool_prefix("claude.ai Datadog (2)"),
            "mcp__claude_ai_Datadog_2"
        );
        assert_eq!(
            tool_prefix("claude.ai Zoom for Claude"),
            "mcp__claude_ai_Zoom_for_Claude"
        );
    }

    #[test]
    fn reads_a_health_line() {
        let got = parse_line("linear: https://mcp.linear.app/mcp (HTTP) - ✔ Connected").unwrap();
        assert_eq!(got.name, "linear");
        assert_eq!(got.tool_prefix, "mcp__linear");
        assert_eq!(got.detail, "https://mcp.linear.app/mcp (HTTP)");
        assert!(got.connected);
    }

    #[test]
    fn spots_a_server_that_needs_authenticating() {
        let got =
            parse_line("claude.ai Gong: https://mcp.gong.io/mcp - ! Needs authentication").unwrap();
        assert_eq!(got.name, "claude.ai Gong");
        assert!(!got.connected);
    }

    #[test]
    fn ignores_chatter() {
        assert!(parse_line("Checking MCP server health…").is_none());
        assert!(parse_line("").is_none());
    }
}
