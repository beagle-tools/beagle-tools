//! Reading a slice of a file at a commit. The diff only carries the
//! lines a PR changed, but a reviewer with a checkout cites any line it
//! likes — so a comment anchored outside the hunks has to fetch its
//! context rather than claim the code is gone.

use serde::Serialize;
use tandem_core::TandemError;
use tauri::State;

use crate::commands::parse_repo;
use crate::state::AppState;

/// Lines around an anchor, with the number the first one carries so the
/// caller can label them.
#[derive(Debug, Clone, Serialize)]
pub struct FileExcerpt {
    pub start_line: u64,
    pub lines: Vec<String>,
}

/// Enough to read the anchor in context without pulling a whole file
/// into a popover.
const MAX_RADIUS: u64 = 40;

#[tauri::command]
pub async fn get_file_excerpt(
    state: State<'_, AppState>,
    repo: String,
    path: String,
    git_ref: String,
    line: u64,
    radius: u64,
) -> Result<FileExcerpt, TandemError> {
    let repo = parse_repo(&repo)?;
    let client = state.github_client().await?;
    let (content, _blob_sha) = client.get_file(&repo, &path, &git_ref).await?;

    let radius = radius.min(MAX_RADIUS);
    let all: Vec<&str> = content.lines().collect();
    if all.is_empty() {
        return Ok(FileExcerpt {
            start_line: 1,
            lines: Vec::new(),
        });
    }
    // 1-based line numbers in, 0-based slice out.
    let anchor = usize::try_from(line.max(1) - 1).unwrap_or(0);
    let radius = usize::try_from(radius).unwrap_or(0);
    let start = anchor.saturating_sub(radius);
    let end = (anchor + radius + 1).min(all.len());
    if start >= all.len() {
        return Err(TandemError::Config(format!(
            "{path} has {} lines; line {line} is past the end",
            all.len()
        )));
    }
    Ok(FileExcerpt {
        start_line: u64::try_from(start).unwrap_or(0) + 1,
        lines: all[start..end].iter().map(|l| (*l).to_owned()).collect(),
    })
}
