//! The agents screen (`A`): daemon status from the background poller,
//! selection, and start/stop over the `beagle-agentd` control socket.

use super::{App, Screen};

impl App {
    /// The active top-level screen.
    pub(crate) fn screen(&self) -> Screen {
        self.screen
    }

    /// The latest daemon status for the agents screen.
    pub(crate) fn agents_status(&self) -> &crate::agentd::AgentsStatus {
        &self.agents_status
    }

    /// Records a daemon status update from the background poller, keeping the
    /// selection within the (possibly changed) agent list.
    pub(crate) fn set_agents_status(&mut self, status: crate::agentd::AgentsStatus) {
        self.agents_status = status;
        let count = self.agent_snapshots().len();
        self.agents_selected = self.agents_selected.min(count.saturating_sub(1));
    }

    /// The highlighted agent index on the agents screen.
    pub(crate) fn agents_selected(&self) -> usize {
        self.agents_selected
    }

    /// The current agent snapshots, or an empty slice unless connected.
    pub(super) fn agent_snapshots(&self) -> &[crate::agentd::AgentSnapshot] {
        if let crate::agentd::AgentsStatus::Connected(snapshot) = &self.agents_status {
            &snapshot.agents
        } else {
            &[]
        }
    }

    /// Moves the agents-screen selection one row, clamped to the list.
    pub(crate) fn move_agent_selection(&mut self, forward: bool) {
        let count = self.agent_snapshots().len();
        if count == 0 {
            self.agents_selected = 0;
            return;
        }
        let last = count - 1;
        self.agents_selected = if forward {
            (self.agents_selected + 1).min(last)
        } else {
            self.agents_selected.saturating_sub(1)
        };
    }

    /// The highlighted agent's snapshot, if the daemon is connected.
    pub(super) fn selected_agent(&self) -> Option<&crate::agentd::AgentSnapshot> {
        self.agent_snapshots().get(self.agents_selected)
    }

    /// Starts or stops the highlighted agent over the control socket, then
    /// refreshes. A no-op when nothing is selected.
    pub(crate) fn toggle_selected_agent(&mut self) {
        let Some((id, running)) = self.selected_agent().map(|a| (a.id.clone(), a.running)) else {
            return;
        };
        let result = if running {
            crate::agentd::stop_agent(&id)
        } else {
            crate::agentd::start_agent(&id)
        };
        match result {
            Ok(()) => {
                let verb = if running { "stopped" } else { "started" };
                self.status = Some(format!("{id}: {verb}"));
                if let Ok(snapshot) = crate::agentd::fetch_status() {
                    self.set_agents_status(crate::agentd::AgentsStatus::Connected(snapshot));
                }
            }
            Err(err) => self.status = Some(format!("agent control failed: {err}")),
        }
    }

    /// The newest session log for the highlighted agent, or `None` (leaving a
    /// status hint) if there is nothing to open.
    pub(crate) fn selected_agent_log(&mut self) -> Option<std::path::PathBuf> {
        let id = self.selected_agent()?.id.clone();
        if let Some(path) = crate::agentd::latest_log(&id) {
            return Some(path);
        }
        self.status = Some(format!("{id}: no session logs yet"));
        None
    }

    /// Toggles between the RCA browser and the agents screen. The RCA screen's
    /// own state (selection, tab, focus, scroll) is left untouched, so
    /// switching back lands exactly where it was.
    pub(crate) fn toggle_screen(&mut self) {
        self.screen = match self.screen {
            Screen::Rcas => Screen::Agents,
            Screen::Agents => Screen::Rcas,
        };
    }
}
