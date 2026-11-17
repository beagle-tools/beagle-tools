//! GitHub actions from the TUI: the confirmation they wait on, and the
//! background `gh` jobs that carry them out.

use super::App;
use crate::actions::{self, MergeMethod, MergePlan};
use crate::model::RecapId;

/// A GitHub action waiting for the user's confirmation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Confirm {
    /// Draft → ready for review (`undo`: ready → draft).
    Ready {
        id: String,
        url: String,
        label: String,
        undo: bool,
    },
    /// Close without merging, then archive the recap.
    Close {
        id: String,
        url: String,
        label: String,
    },
    /// Merge (or enable auto-merge) with the chosen method.
    Merge {
        id: String,
        url: String,
        label: String,
        plan: MergePlan,
        choice: usize,
    },
}

/// A finished background `gh` job.
#[derive(Debug)]
pub(crate) enum Job {
    /// The merge-readiness check came back.
    MergeChecked {
        id: String,
        url: String,
        label: String,
        result: std::result::Result<MergePlan, String>,
    },
    /// An action that changes GitHub finished.
    Done {
        id: String,
        label: String,
        closed: bool,
        result: std::result::Result<String, String>,
    },
}

impl App {
    /// Runs `job` on a background thread and reports through `jobs_rx`. With
    /// actions disabled (tests), only records `what`.
    pub(super) fn spawn_job(&mut self, what: String, job: impl FnOnce() -> Job + Send + 'static) {
        if !self.actions_enabled {
            self.requested.push(what);
            return;
        }
        self.busy = Some(what);
        let tx = self.jobs_tx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(job()); // the TUI quit before the job finished
        });
    }

    /// Takes a finished background job.
    pub(crate) fn on_job(&mut self, job: Job) {
        self.busy = None;
        match job {
            Job::MergeChecked {
                id,
                url,
                label,
                result,
            } => match result {
                Ok(plan) => {
                    self.confirm = Some(Confirm::Merge {
                        id,
                        url,
                        label,
                        plan,
                        choice: 0,
                    });
                }
                Err(reason) => self.status = Some(format!("can't merge {label}: {reason}")),
            },
            Job::Done {
                id,
                label,
                closed,
                result,
            } => match result {
                Ok(message) => {
                    self.status = Some(format!("{label}: {message}"));
                    if closed {
                        // A closed PR's recap leaves the list (it's kept in
                        // the archive).
                        match RecapId::new(&id).and_then(|rid| self.store.archive(&rid)) {
                            Ok(_) => {
                                self.status = Some(format!(
                                    "{label}: closed and archived (A shows the archive)"
                                ));
                            }
                            Err(e) => {
                                self.status =
                                    Some(format!("{label}: closed, but archiving failed: {e}"));
                            }
                        }
                    }
                    self.repoll = true;
                    self.reload();
                }
                Err(e) => self.status = Some(format!("{label}: gh refused: {e}")),
            },
        }
    }

    /// Runs the confirmed action.
    pub(super) fn run_confirmed(&mut self) {
        let Some(confirm) = self.confirm.take() else {
            return;
        };
        match confirm {
            Confirm::Ready {
                id,
                url,
                label,
                undo,
            } => {
                let what = format!(
                    "{} {label}",
                    if undo {
                        "moving to draft"
                    } else {
                        "marking ready"
                    }
                );
                let l = label.clone();
                self.spawn_job(what, move || Job::Done {
                    id,
                    label: l,
                    closed: false,
                    result: actions::set_ready(&url, undo),
                });
            }
            Confirm::Close { id, url, label } => {
                let l = label.clone();
                self.spawn_job(format!("closing {label}"), move || Job::Done {
                    id,
                    label: l,
                    closed: true,
                    result: actions::close(&url),
                });
            }
            Confirm::Merge {
                id,
                url,
                label,
                plan,
                choice,
            } => {
                let method = plan
                    .methods
                    .get(choice)
                    .copied()
                    .unwrap_or(MergeMethod::Squash);
                let l = label.clone();
                self.spawn_job(
                    format!(
                        "{} {label} ({})",
                        if plan.auto {
                            "enabling auto-merge on"
                        } else {
                            "merging"
                        },
                        method.label()
                    ),
                    move || Job::Done {
                        id,
                        label: l,
                        closed: false,
                        result: actions::merge(&url, method, &plan),
                    },
                );
            }
        }
    }
}
