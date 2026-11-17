//! `R`: run a job, or resume a chain at a failed step — behind a floating
//! confirmation pane, because both start real (often paid) work.
//!
//! On the jobs page `R` starts the job under the cursor from its entry
//! point. On a job's runs screen it resumes the chain at the selected step
//! when that step failed, timed out or was killed (`agent_core::resume`),
//! and otherwise starts the job again from the top. The pane says which.

use agent_core::types::Trigger;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph, Wrap};
use ratatui::Frame;

use crossterm::event::KeyCode;

use crate::app::App;

/// What a confirmed pane does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pending {
    /// Start `project/name` from its entry point.
    Start { project: String, name: String },
    /// Queue the failed run `run_id` again in its chain.
    Resume { run_id: String, step: String },
}

/// The open pane: what it will do, and the lines that explain it.
#[derive(Debug, Clone)]
pub struct Confirm {
    pub title: String,
    pub body: Vec<String>,
    pub pending: Pending,
}

impl App {
    /// `R`: open the pane for whatever `R` means where the cursor is.
    pub fn ask_run(&mut self) {
        if self.runs_open() {
            if let Some(run) = self.dag_selected_run().cloned() {
                if agent_core::resume::resumable(&run) {
                    self.confirm = Some(Confirm {
                        title: "RESUME".to_owned(),
                        body: vec![
                            format!("Resume the chain at {}?", run.agent),
                            String::new(),
                            format!("It {} — a new run of that step reads", run.state.as_str()),
                            "the same handover, with the same params and model,".to_owned(),
                            "and the rest of the chain carries on from there.".to_owned(),
                        ],
                        pending: Pending::Resume {
                            run_id: run.id,
                            step: run.agent,
                        },
                    });
                    return;
                }
            }
        }
        let Some(spec) = self.dag_selected_agent().cloned() else {
            self.set_status("no job selected");
            return;
        };
        let missing: Vec<&str> = spec
            .params
            .iter()
            .filter(|p| p.required && p.default.is_none())
            .map(|p| p.name.as_str())
            .collect();
        if !missing.is_empty() {
            self.set_status(&format!(
                "{} needs {} — run it with: agentctl run {}/{} --param …",
                spec.name,
                missing.join(", "),
                spec.project,
                spec.name
            ));
            return;
        }
        let again = if self.runs_open() {
            " again from the start"
        } else {
            ""
        };
        self.confirm = Some(Confirm {
            title: "RUN".to_owned(),
            body: vec![
                format!("Run {}/{}{again}?", spec.project, spec.name),
                String::new(),
                "Its whole chain runs, as the schedule would run it.".to_owned(),
            ],
            pending: Pending::Start {
                project: spec.project,
                name: spec.name,
            },
        });
    }

    /// Keys while the pane is open: enter / y runs it, esc / q / n closes
    /// it, and every other key is swallowed so nothing moves underneath.
    pub fn confirm_key(&mut self, code: KeyCode) {
        let Some(confirm) = self.confirm.take() else {
            return;
        };
        match code {
            KeyCode::Enter | KeyCode::Char('y') => self.run_pending(confirm.pending),
            KeyCode::Esc | KeyCode::Char('q' | 'n') => {}
            _ => self.confirm = Some(confirm),
        }
    }

    fn run_pending(&mut self, pending: Pending) {
        let result = match &pending {
            Pending::Start { project, name } => self.with_store(|store| {
                store.create_pending_run(project, name, Trigger::Manual, None, None, None)
            }),
            Pending::Resume { run_id, .. } => agent_core::paths::data_dir().and_then(|data| {
                self.with_store(|store| agent_core::resume::resume_run(store, run_id, &data))
            }),
        };
        match (result, pending) {
            (Ok(run), Pending::Start { .. }) => self.set_status(&format!("started {}", run.id)),
            (Ok(run), Pending::Resume { step, .. }) => {
                self.set_status(&format!("resumed at {step}: {}", run.id));
            }
            (Err(e), _) => self.set_status(&format!("cannot run: {e}")),
        }
        self.refresh();
    }
}

pub fn draw(frame: &mut Frame<'_>, confirm: &Confirm) {
    let mut lines: Vec<Line<'_>> = confirm.body.iter().map(|l| Line::raw(l.as_str())).collect();
    lines.push(Line::raw(""));
    lines.push(Line::from(vec![
        Span::styled(
            " enter ",
            Style::new()
                .fg(Color::Black)
                .bg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" run   "),
        Span::styled(" esc ", Style::new().add_modifier(Modifier::REVERSED)),
        Span::raw(" cancel"),
    ]));
    let width = confirm
        .body
        .iter()
        .map(|l| l.chars().count())
        .max()
        .unwrap_or(0)
        .clamp(40, 72);
    let w = u16::try_from(width + 4).unwrap_or(76);
    let h = u16::try_from(lines.len() + 2).unwrap_or(u16::MAX);
    let area = centered(w, h, frame.area());
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(lines)
            .block(crate::dag::pane_block(&confirm.title))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn centered(width: u16, height: u16, area: Rect) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    Rect {
        x: area.x + (area.width - w) / 2,
        y: area.y + (area.height - h) / 2,
        width: w,
        height: h,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open(app: &mut App) {
        app.confirm = Some(Confirm {
            title: "RUN".to_owned(),
            body: vec!["Run p/x?".to_owned()],
            pending: Pending::Start {
                project: "p".to_owned(),
                name: "x".to_owned(),
            },
        });
    }

    #[test]
    fn other_keys_leave_the_pane_open_and_esc_closes_it() {
        let mut app = App::default();
        open(&mut app);
        app.confirm_key(KeyCode::Char('j'));
        assert!(app.confirm.is_some(), "j must not move anything underneath");
        app.confirm_key(KeyCode::Esc);
        assert!(app.confirm.is_none());
        open(&mut app);
        app.confirm_key(KeyCode::Char('n'));
        assert!(app.confirm.is_none());
    }

    #[test]
    fn r_with_nothing_selected_opens_no_pane() {
        let mut app = App::default();
        app.ask_run();
        assert!(app.confirm.is_none());
    }
}
