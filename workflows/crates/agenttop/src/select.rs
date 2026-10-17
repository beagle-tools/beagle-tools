//! What is selected on the dashboard, and what you can do with it.
//!
//! Split out of [`crate::app`] to keep that file inside the 500-line limit;
//! it is the same inherent impl on [`App`].

use agent_core::types::Run;

use crate::app::App;

impl App {
    /// Total selectable rows across the ACTIVE and RECENT panes.
    pub fn row_count(&self) -> usize {
        self.active.len() + self.recent.len()
    }

    /// The run under the cursor: ACTIVE rows first, then RECENT rows.
    pub fn selected_run(&self) -> Option<&Run> {
        if self.selected < self.active.len() {
            self.active.get(self.selected)
        } else {
            self.recent.get(self.selected - self.active.len())
        }
    }

    /// `D` — jump from a run on the activity page to that same run inside
    /// its job's runs screen.
    ///
    /// Walks up `parent_run_id` to the chain root first, because the root's
    /// agent IS the job: a `linear-report` row belongs to `linear`.
    pub fn focus_dag_on_selection(&mut self) {
        let Some(selected) = self.selected_run() else {
            return;
        };
        let by_id: std::collections::HashMap<&str, &Run> = self
            .active
            .iter()
            .chain(self.recent.iter())
            .map(|r| (r.id.as_str(), r))
            .collect();
        let mut root = selected;
        // Bounded: a cycle would otherwise hang the UI, and the depth here is
        // single digits in practice.
        for _ in 0..32 {
            match root.parent_run_id.as_deref().and_then(|p| by_id.get(p)) {
                Some(parent) => root = parent,
                None => break,
            }
        }
        let selected_id = selected.id.clone();
        self.job_open = Some((root.project.clone(), root.agent.clone()));
        self.show_dag = true;
        // Land on the run you pressed D from, not the chain's root — that
        // is usually the script stage you did not mean. Falls back to the
        // first step if history no longer holds it.
        self.job_step_selected = self
            .job_steps()
            .iter()
            .position(|r| r.id == selected_id)
            .unwrap_or(0);
    }

    pub fn move_selection(&mut self, down: bool) {
        let len = self.row_count();
        if len == 0 {
            self.selected = 0;
        } else if down {
            self.selected = (self.selected + 1).min(len - 1);
        } else {
            self.selected = self.selected.saturating_sub(1);
        }
    }
}

#[cfg(test)]
// Tests should fail loudly on a broken fixture; unwrapping is the clearest way.
#[allow(clippy::unwrap_used)]
mod tests {
    use agent_core::types::{RunState, Trigger};
    use chrono::Utc;

    use super::*;

    fn run(id: &str, state: RunState) -> Run {
        Run {
            id: id.to_owned(),
            project: "p".to_owned(),
            agent: "a".to_owned(),
            state,
            trigger: Trigger::Manual,
            parent_run_id: None,
            params: None,
            tmux_session: None,
            log_path: None,
            workdir: None,
            model: None,
            created_at: Utc::now(),
            started_at: None,
            finished_at: None,
            exit_code: None,
            cost_usd: None,
            num_turns: None,
            usage_json: None,
        }
    }

    #[test]
    fn focusing_walks_up_to_the_chain_root() {
        // Pressing D on a leaf must pin the ROOT, or the stages above it
        // vanish — and on a linear-report row that is the whole ticket.
        let mut app = App::new();
        let mut root = run("root", RunState::Succeeded);
        root.parent_run_id = None;
        let mut mid = run("mid", RunState::Succeeded);
        mid.parent_run_id = Some("root".to_owned());
        let mut leaf = run("leaf", RunState::Succeeded);
        leaf.parent_run_id = Some("mid".to_owned());
        app.recent = vec![root, mid, leaf];
        app.history = app.recent.clone();

        app.selected = 2; // the leaf
        app.focus_dag_on_selection();
        // The ROOT's agent is the job, and the cursor lands on the leaf.
        assert_eq!(app.job_open, Some(("p".to_owned(), "a".to_owned())));
        assert!(app.show_dag);
        assert_eq!(app.dag_selected_run().map(|r| r.id.as_str()), Some("leaf"));

        // Pressing it on the root opens the same job, on the root.
        app.job_open = None;
        app.selected = 0;
        app.focus_dag_on_selection();
        assert_eq!(app.job_open, Some(("p".to_owned(), "a".to_owned())));
        assert_eq!(app.dag_selected_run().map(|r| r.id.as_str()), Some("root"));
    }

    #[test]
    fn d_lands_the_cursor_on_the_run_you_pressed_it_from() {
        // Not on the root: the root is usually the script stage, and the row
        // you pressed D on is the one you wanted to look at.
        let mut app = App::new();
        let mut root = run("root", RunState::Succeeded);
        root.parent_run_id = None;
        let mut leaf = run("leaf", RunState::Succeeded);
        leaf.parent_run_id = Some("root".to_owned());
        app.recent = vec![root, leaf];
        app.history = app.recent.clone();
        app.selected = 1; // the leaf
        app.focus_dag_on_selection();
        assert!(app.job_open.is_some(), "D should open the run's job");
        assert_eq!(
            app.dag_selected_run().map(|r| r.id.as_str()),
            Some("leaf"),
            "cursor should sit on the run D was pressed from"
        );
    }

    #[test]
    fn the_runs_cursor_moves_over_steps_and_never_stops_on_a_partition() {
        // Two runs of job `a`: root1 -> leaf1, and root2 alone. Three
        // steps, two partition headers. The cursor must only ever be on a
        // step, and must not run past the last one.
        let mut app = App::new();
        let mut root1 = run("root1", RunState::Succeeded);
        root1.created_at = Utc::now() - chrono::Duration::hours(2);
        let mut leaf1 = run("leaf1", RunState::Succeeded);
        leaf1.parent_run_id = Some("root1".to_owned());
        leaf1.agent = "step".to_owned();
        let root2 = run("root2", RunState::Succeeded);
        app.history = vec![root2, root1, leaf1];
        app.job_open = Some(("p".to_owned(), "a".to_owned()));

        assert_eq!(app.job_steps().len(), 3);
        let mut seen = Vec::new();
        for _ in 0..6 {
            seen.push(app.dag_selected_run().map(|r| r.id.clone()));
            app.dag_move(true);
        }
        assert_eq!(seen[0].as_deref(), Some("root2"), "newest run first");
        assert!(seen.iter().all(Option::is_some), "never on a header");
        assert_eq!(
            seen.last().unwrap().as_deref(),
            Some("leaf1"),
            "clamps at the last step"
        );
    }

    #[test]
    fn with_nothing_declared_the_jobs_cursor_stays_put() {
        let mut app = App::new();
        app.dag_move(true);
        app.dag_move(false);
        assert_eq!(app.dag_selected, 0);
        assert!(app.selected_job().is_none());
    }

    #[test]
    fn focusing_survives_a_broken_parent_link() {
        // A parent pruned out of view must not hang the walk or panic.
        let mut app = App::new();
        let mut orphan = run("orphan", RunState::Succeeded);
        orphan.parent_run_id = Some("long-gone".to_owned());
        app.recent = vec![orphan];
        app.selected = 0;
        app.focus_dag_on_selection();
        assert_eq!(app.job_open, Some(("p".to_owned(), "a".to_owned())));
    }
}
