//! What a reply run gets to see besides the thread it's answering: the
//! other conversations on the PR, and the slice of diff under
//! discussion. Split from threads.rs for the 400-line rule.

use tandem_agents::context::ThreadMessage;
use tandem_agents::conversation;
use tandem_cache::ReviewStore;
use tandem_core::review::LocalComment;
use tandem_core::TandemError;

/// Every other conversation on this PR — GitHub review threads, plain
/// PR comments, and Tandem's own local threads — newest first, with the
/// thread being answered left out. All of it is already in the cache;
/// the agent was only ever missing it because nobody passed it along.
pub(crate) fn sibling_threads(
    cache: &tandem_cache::Cache,
    detail: &tandem_core::github::PrDetail,
    root: &LocalComment,
) -> Result<Vec<conversation::ConversationThread>, TandemError> {
    use std::collections::BTreeMap;

    let mut out: Vec<(
        chrono::DateTime<chrono::Utc>,
        conversation::ConversationThread,
    )> = Vec::new();

    // GitHub: roots are comments that reply to nothing; everything else
    // hangs off one. A root the local thread already quotes is skipped.
    let mut replies: BTreeMap<u64, Vec<&tandem_core::github::GithubComment>> = BTreeMap::new();
    for comment in &detail.comments {
        if let Some(parent) = comment.in_reply_to_id {
            replies.entry(parent).or_default().push(comment);
        }
    }
    for comment in &detail.comments {
        if comment.in_reply_to_id.is_some() || Some(comment.id) == root.github_comment_id {
            continue;
        }
        let anchor = match (&comment.path, comment.line.or(comment.original_line)) {
            (Some(path), Some(line)) => format!("{path}:{line} (github)"),
            (Some(path), None) => format!("{path} (github, outdated)"),
            _ => "the PR itself (github)".to_owned(),
        };
        let mut messages = vec![ThreadMessage {
            author: format!("{} (github)", comment.author.login),
            body: comment.body.clone(),
        }];
        for reply in replies.get(&comment.id).into_iter().flatten() {
            messages.push(ThreadMessage {
                author: format!("{} (github)", reply.author.login),
                body: reply.body.clone(),
            });
        }
        out.push((
            comment.created_at,
            conversation::ConversationThread { anchor, messages },
        ));
    }

    // Tandem's own threads, minus the one being replied to.
    let local = cache.list_comments(&root.repo, root.pr_number)?;
    let mut local_replies: BTreeMap<String, Vec<&LocalComment>> = BTreeMap::new();
    for comment in &local {
        if let Some(parent) = &comment.parent_id {
            local_replies
                .entry(parent.clone())
                .or_default()
                .push(comment);
        }
    }
    for comment in &local {
        if comment.parent_id.is_some() || comment.id == root.id {
            continue;
        }
        let anchor = if comment.path.is_empty() || comment.line == 0 {
            "the PR itself (tandem)".to_owned()
        } else {
            format!("{}:{} (tandem)", comment.path, comment.line)
        };
        let mut messages = vec![ThreadMessage {
            author: comment.author_name.clone(),
            body: comment.body.clone(),
        }];
        for reply in local_replies.get(&comment.id).into_iter().flatten() {
            messages.push(ThreadMessage {
                author: reply.author_name.clone(),
                body: reply.body.clone(),
            });
        }
        out.push((
            comment.created_at,
            conversation::ConversationThread { anchor, messages },
        ));
    }

    out.sort_by_key(|entry| std::cmp::Reverse(entry.0));
    Ok(out.into_iter().map(|(_, thread)| thread).collect())
}

/// Pull one file's section out of a raw multi-file unified diff; falls
/// back to the whole diff when the file isn't found.
pub(crate) fn extract_file_diff(raw: &str, path: &str) -> String {
    let mut out = String::new();
    let mut in_file = false;
    for line in raw.lines() {
        if line.starts_with("diff --git ") {
            in_file = line.contains(&format!(" b/{path}")) || line.contains(&format!("a/{path} "));
        }
        if in_file {
            out.push_str(line);
            out.push('\n');
        }
    }
    if out.is_empty() {
        raw.to_owned()
    } else {
        out
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::extract_file_diff;

    #[test]
    fn extracts_single_file_section() {
        let raw = "diff --git a/a.rs b/a.rs\n--- a/a.rs\n+++ b/a.rs\n@@ -1 +1 @@\n-x\n+y\n\
                   diff --git a/b.ts b/b.ts\n--- a/b.ts\n+++ b/b.ts\n@@ -1 +1 @@\n-p\n+q\n";
        let got = extract_file_diff(raw, "b.ts");
        assert!(got.contains("+++ b/b.ts"));
        assert!(!got.contains("a.rs"));
    }

    #[test]
    fn falls_back_to_full_diff() {
        let raw = "diff --git a/a.rs b/a.rs\n+x\n";
        assert_eq!(extract_file_diff(raw, "missing.py"), raw);
    }
}
