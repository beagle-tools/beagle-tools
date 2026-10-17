//! The rest of what's been said on a PR, rendered for an agent that's
//! replying in one thread. Tandem already has every GitHub comment and
//! every local thread in its cache — an agent that answers "I can't see
//! the other threads" is being starved of context it could have had.

use crate::context::ThreadMessage;
use std::fmt::Write as _;

/// Keep the sibling conversation from crowding out the thread actually
/// being answered. Oldest threads drop first.
const MAX_THREADS: usize = 12;
const PER_MESSAGE_CHARS: usize = 1_200;
const TOTAL_CHARS: usize = 12_000;

/// One other thread on the PR, rendered as context for a reply run.
#[derive(Debug, Clone)]
pub struct ConversationThread {
    /// Where it hangs: "src/x.rs:42", "pr comment", "resolved".
    pub anchor: String,
    /// Messages in the thread, oldest first.
    pub messages: Vec<ThreadMessage>,
}

fn clip(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.chars().count() <= PER_MESSAGE_CHARS {
        return trimmed.to_owned();
    }
    let kept: String = trimmed.chars().take(PER_MESSAGE_CHARS).collect();
    format!("{kept}\n[…truncated]")
}

/// The prompt section, or nothing when this is the only thread.
pub fn render(threads: &[ConversationThread]) -> String {
    if threads.is_empty() {
        return String::new();
    }
    let mut out = String::from(
        "## The rest of the conversation on this PR\n\
         Other threads, from GitHub and from Tandem, newest first. They \
         are context only — do NOT reply to them, and do not repeat a \
         point someone has already made. If one of them answers the \
         question you're being asked, say so and cite it.\n\n",
    );
    let mut budget = TOTAL_CHARS;
    let mut shown = 0;
    for thread in threads.iter().take(MAX_THREADS) {
        if budget == 0 {
            break;
        }
        let mut block = format!("### thread on {}\n", thread.anchor);
        for msg in &thread.messages {
            let _ = writeln!(block, "- **{}**: {}", msg.author, clip(&msg.body));
        }
        block.push('\n');
        let cost = block.chars().count();
        if cost > budget && shown > 0 {
            break;
        }
        budget = budget.saturating_sub(cost);
        out.push_str(&block);
        shown += 1;
    }
    let omitted = threads.len().saturating_sub(shown);
    if omitted > 0 {
        let _ = write!(
            out,
            "[{omitted} older thread(s) not shown — ask the reviewer if you need them]\n\n"
        );
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn thread(anchor: &str, body: &str) -> ConversationThread {
        ConversationThread {
            anchor: anchor.to_owned(),
            messages: vec![ThreadMessage {
                author: "matt".to_owned(),
                body: body.to_owned(),
            }],
        }
    }

    #[test]
    fn nothing_to_say_renders_nothing() {
        assert_eq!(render(&[]), "");
    }

    #[test]
    fn renders_threads_with_their_anchors() {
        let out = render(&[thread("src/x.rs:42", "this needs a guard")]);
        assert!(out.contains("thread on src/x.rs:42"));
        assert!(out.contains("this needs a guard"));
        assert!(out.contains("do NOT reply to them"));
    }

    #[test]
    fn long_messages_are_clipped() {
        let out = render(&[thread("pr comment", &"x".repeat(PER_MESSAGE_CHARS + 50))]);
        assert!(out.contains("[…truncated]"));
    }

    #[test]
    fn past_the_thread_cap_the_rest_are_counted_not_dropped_silently() {
        let threads: Vec<_> = (0..MAX_THREADS + 3)
            .map(|i| thread(&format!("src/f{i}.rs:1"), "hm"))
            .collect();
        let out = render(&threads);
        assert!(out.contains("3 older thread(s) not shown"));
    }
}
