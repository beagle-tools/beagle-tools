//! The TUI: application state, the event loop, key handling, and drawing.
//!
//! All state lives in [`App`] and changes only on a key press or a
//! filesystem event. Drawing (`view`) is a pure function of `App` apart from
//! scroll clamping, so every state transition is testable without a
//! terminal.
//!
//! Two screens, like beagle's list → workspace flow:
//!
//! - **Projects**: every project on the left, the selected project's goal and
//!   progress on the right.
//! - **Tasks**: one project's ordered tasks on the left, the selected task (or
//!   the project doc, or the activity log) on the right.

mod event_loop;
mod keys;
mod view;

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::io::ErrorKind;
use std::path::PathBuf;

use beagle_tui::{DefaultWidth, SideResize};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Text};

use crate::error::{Error, Result};
use crate::model::{Slug, TaskStatus};
use crate::store::{self, Project, Store, Task};
use crate::{frontmatter, markdown, transcript, transcript_view};

/// The top-level screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    /// Every project.
    Projects,
    /// One project's tasks.
    Tasks,
}

/// Which pane on the Tasks screen receives `j`/`k`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    /// The task list.
    List,
    /// The content pane (scrolling).
    Content,
}

/// The content tabs on the Tasks screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    /// The selected task's file, minus its handoff sections.
    Task,
    /// The selected task's `## Outcome` and `## Next steps`: what was done
    /// and what should happen next.
    Outcome,
    /// The task's attached Claude sessions, rendered from scout's copy.
    Session,
    /// The project's `project.md`.
    Project,
    /// The project's `log.md`.
    Log,
}

impl Tab {
    /// Every tab, in display order (`1`–`5`).
    pub const ALL: [Self; 5] = [
        Self::Task,
        Self::Outcome,
        Self::Session,
        Self::Project,
        Self::Log,
    ];

    /// The tab's label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Task => "Task",
            Self::Outcome => "Outcome",
            Self::Session => "Session",
            Self::Project => "Project",
            Self::Log => "Log",
        }
    }

    fn index(self) -> usize {
        Self::ALL.iter().position(|t| *t == self).unwrap_or(0)
    }

    fn step(self, forward: bool) -> Self {
        let n = Self::ALL.len();
        let i = if forward {
            (self.index() + 1) % n
        } else {
            (self.index() + n - 1) % n
        };
        Self::ALL[i]
    }
}

/// Which part of a file the content pane shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    /// The whole body.
    Whole,
    /// A task body without its handoff sections.
    TaskMain,
    /// Only a task's handoff sections (Outcome, Next steps).
    Handoff,
    /// A task's attached sessions.
    Session,
}

/// The rendered content pane, cached per (file, view) and re-read on reload.
#[derive(Debug)]
struct CachedPane {
    path: PathBuf,
    view: View,
    text: Text<'static>,
    /// Where each line starts once wrapped, at one width: computed once per
    /// content or width change, so a frame only wraps the lines it shows.
    layout: Option<WrapLayout>,
}

/// Wrapped-row offsets of a pane's lines at one width.
#[derive(Debug)]
pub(super) struct WrapLayout {
    /// The width the offsets were computed for.
    pub(super) width: u16,
    /// `starts[i]` is the first row of line `i`; the last entry is the
    /// total number of rows.
    pub(super) starts: Vec<usize>,
}

/// The task on screen as `(project dir, attached session ids)`, shared
/// between the UI thread and the session pipeline thread.
type OnScreen = std::sync::Arc<std::sync::Mutex<Option<(PathBuf, Vec<String>)>>>;

/// The whole TUI state.
#[derive(Debug)]
pub struct App {
    store: Store,
    projects: Vec<Project>,
    warnings: Vec<String>,
    screen: Screen,
    focus: Focus,
    tab: Tab,
    project_sel: usize,
    task_sel: usize,
    /// Content scroll in rendered (wrapped) lines; `usize::MAX` means "the
    /// bottom" and is clamped by the draw pass. `usize`, not `u16`: a long
    /// session wraps to well over 65k rows.
    scroll: usize,
    /// Set by the draw pass: the furthest the content can scroll.
    max_scroll: usize,
    /// Set by the draw pass: the content pane's height.
    viewport_height: u16,
    /// Tail mode: keep the content pinned to its bottom across reloads.
    follow: bool,
    help: bool,
    /// One-shot status-bar message, cleared on the next key press.
    status: Option<String>,
    /// Animation counter for the working spinner, bumped every loop turn.
    tick: usize,
    /// The task on screen, shared with the session pipeline thread so it
    /// keeps that task's sessions fresh too.
    on_screen: OnScreen,
    pane: Option<CachedPane>,
    /// The left list's width and `w`'s resize mode (not saved across runs).
    side: SideResize,
}

impl App {
    /// Loads the store and starts on the Projects screen.
    ///
    /// # Errors
    /// [`Error::Io`] if the store root exists but cannot be listed.
    pub fn new(store: Store) -> Result<Self> {
        let listing = store.list()?;
        Ok(Self {
            store,
            projects: listing.projects,
            warnings: listing.warnings,
            screen: Screen::Projects,
            focus: Focus::List,
            tab: Tab::Task,
            project_sel: 0,
            task_sel: 0,
            scroll: 0,
            max_scroll: 0,
            viewport_height: 0,
            follow: false,
            help: false,
            status: None,
            tick: 0,
            on_screen: std::sync::Arc::default(),
            pane: None,
            side: SideResize::new(
                "list",
                DefaultWidth::Fraction {
                    divisor: 3,
                    min: 28,
                    max: 48,
                },
            ),
        })
    }

    fn selected_project(&self) -> Option<&Project> {
        self.projects.get(self.project_sel)
    }

    fn selected_task(&self) -> Option<&Task> {
        self.selected_project()?.tasks.get(self.task_sel)
    }

    fn select_project(&mut self, index: usize) {
        self.project_sel = index.min(self.projects.len().saturating_sub(1));
    }

    fn select_task(&mut self, index: usize) {
        let len = self.selected_project().map_or(0, |p| p.tasks.len());
        self.task_sel = index.min(len.saturating_sub(1));
    }

    fn reset_scroll(&mut self) {
        self.scroll = if self.follow { usize::MAX } else { 0 };
    }

    /// Enters the selected project, landing on the task being worked (or the
    /// next one to do) so "what is the agent doing?" is one keypress away.
    fn open_project(&mut self) {
        let Some(project) = self.selected_project() else {
            return;
        };
        self.task_sel = default_task(project);
        self.screen = Screen::Tasks;
        self.focus = Focus::List;
        self.tab = Tab::Task;
    }

    fn back_to_projects(&mut self) {
        self.screen = Screen::Projects;
        self.focus = Focus::List;
    }

    /// The file the content pane shows (what `E` opens).
    fn current_file(&self) -> Option<PathBuf> {
        self.current_view().map(|(path, _)| path)
    }

    /// The file and the part of it the content pane shows.
    fn current_view(&self) -> Option<(PathBuf, View)> {
        let project = self.selected_project()?;
        match (self.screen, self.tab) {
            (Screen::Projects, _) | (Screen::Tasks, Tab::Project) => {
                Some((project.file(), View::Whole))
            }
            (Screen::Tasks, Tab::Task) => self
                .selected_task()
                .map(|t| (t.path.clone(), View::TaskMain)),
            (Screen::Tasks, Tab::Outcome) => self
                .selected_task()
                .map(|t| (t.path.clone(), View::Handoff)),
            (Screen::Tasks, Tab::Session) => self
                .selected_task()
                .map(|t| (t.path.clone(), View::Session)),
            (Screen::Tasks, Tab::Log) => Some((project.log_file(), View::Whole)),
        }
    }

    /// Loads the content pane when what it shows changed, resetting scroll.
    fn ensure_pane(&mut self) {
        let Some((path, view)) = self.current_view() else {
            self.pane = None;
            return;
        };
        if self
            .pane
            .as_ref()
            .is_some_and(|p| p.path == path && p.view == view)
        {
            return;
        }
        let text = load_pane(&path, view);
        self.pane = Some(CachedPane {
            path,
            view,
            text,
            layout: None,
        });
        self.reset_scroll();
        if view == View::Session {
            self.scroll = usize::MAX; // a session reads from its latest activity
        }
    }

    /// Re-reads the store, keeping the selection on the same project and
    /// task. Returns a status-bar announcement of what changed, if anything
    /// worth saying did.
    pub(crate) fn reload(&mut self) -> Option<String> {
        let project_id = self.selected_project().map(|p| p.id.clone());
        let task_id = self.selected_task().map(|t| t.id.clone());
        let known: HashSet<Slug> = self.projects.iter().map(|p| p.id.clone()).collect();
        let before: HashMap<(Slug, String), TaskStatus> = self
            .projects
            .iter()
            .flat_map(|p| {
                p.tasks
                    .iter()
                    .map(|t| ((p.id.clone(), t.id.clone()), t.meta.status))
            })
            .collect();

        match self.store.list() {
            Ok(listing) => {
                self.projects = listing.projects;
                self.warnings = listing.warnings;
            }
            Err(e) => return Some(format!("reload failed: {e}")),
        }

        if let Some(index) = project_id.and_then(|id| self.projects.iter().position(|p| p.id == id))
        {
            self.project_sel = index;
            if let Some(index) =
                task_id.and_then(|id| self.projects[index].tasks.iter().position(|t| t.id == id))
            {
                self.task_sel = index;
            }
        }
        self.select_project(self.project_sel);
        self.select_task(self.task_sel);
        if self.selected_project().is_none() {
            self.back_to_projects();
        }
        if let Some(pane) = &mut self.pane {
            let text = load_pane(&pane.path, pane.view);
            // Sessions only grow at the end: keep the wrapped layout of the
            // lines we already had and wrap just the new ones.
            pane.layout = pane
                .layout
                .take()
                .and_then(|layout| view::extend_layout(layout, &pane.text.lines, &text.lines));
            pane.text = text;
        }

        if let Some(p) = self.projects.iter().find(|p| !known.contains(&p.id)) {
            return Some(format!("new project: {}", p.meta.title));
        }
        self.projects.iter().find_map(|p| {
            p.tasks.iter().find_map(|t| {
                let old = before.get(&(p.id.clone(), t.id.clone()))?;
                (*old != t.meta.status)
                    .then(|| format!("{}: {} → {}", p.meta.title, t.id, t.meta.status.as_str()))
            })
        })
    }
}

mod pane;

use pane::{default_task, load_pane};

#[cfg(test)]
mod tests;
