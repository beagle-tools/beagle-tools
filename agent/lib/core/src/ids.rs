use chrono::Utc;
use uuid::Uuid;

/// Generate a run id: `<agent>-<utc timestamp>-<8 hex chars>`.
///
/// Sortable by creation time, greppable by agent name, unique via the suffix.
/// Also used as the tmux session name (prefixed), so it stays shell-safe.
pub fn new_run_id(agent: &str) -> String {
    let ts = Utc::now().format("%Y%m%d%H%M%S");
    let suffix = Uuid::new_v4().simple().to_string();
    format!("{agent}-{ts}-{}", &suffix[..8])
}

/// The tmux session name for a run — namespaced so `agentd` can tell its own
/// sessions apart from the user's.
pub fn tmux_session_name(run_id: &str) -> String {
    format!("agentd-{run_id}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_ids_are_unique_and_prefixed() {
        let a = new_run_id("digest");
        let b = new_run_id("digest");
        assert!(a.starts_with("digest-"));
        assert_ne!(a, b);
    }

    #[test]
    fn tmux_names_are_namespaced() {
        assert!(tmux_session_name("x-1-2").starts_with("agentd-"));
    }
}
