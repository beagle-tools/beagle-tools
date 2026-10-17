//! Small text utilities backed by the local claude CLI — currently just
//! "polish": fix typos/grammar in a comment draft on explicit request.
//! This never runs automatically; it's a button the user presses.

use std::process::Stdio;
use tandem_core::TandemError;
use tokio::io::AsyncWriteExt;

const POLISH_PROMPT: &str = "Fix spelling, grammar and capitalization in the text after the \
blank line. Preserve the meaning, tone, markdown formatting, code spans/blocks, file paths, \
@mentions and identifiers EXACTLY as written. Return ONLY the corrected text — no preamble, \
no quotes, no code fences around the whole thing.\n\n";

#[tauri::command]
pub async fn polish_text(text: String) -> Result<String, TandemError> {
    if text.trim().is_empty() {
        return Ok(text);
    }
    let mut child = tokio::process::Command::new("claude")
        .env("PATH", beagle_utils::process::augmented_path())
        .args(["-p", "--model", "haiku"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| TandemError::Agent(format!("couldn't run the claude CLI to polish: {e}")))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(format!("{POLISH_PROMPT}{text}").as_bytes())
            .await
            .map_err(TandemError::Io)?;
    }

    let output = tokio::time::timeout(std::time::Duration::from_secs(60), child.wait_with_output())
        .await
        .map_err(|_| TandemError::Agent("polish timed out after 60s".into()))?
        .map_err(TandemError::Io)?;

    if !output.status.success() {
        return Err(TandemError::Agent("claude polish run failed".into()));
    }
    let polished = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if polished.is_empty() {
        return Err(TandemError::Agent(
            "polish returned nothing — draft left as-is".into(),
        ));
    }
    Ok(polished)
}
