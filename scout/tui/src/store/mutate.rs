//! Every change to the store: create, set status, attach, link, log, and
//! the auto-pause sweep. Each one also appends to the project log.

use super::*;

impl Store {
    /// Creates a project in `planning` status from [`PROJECT_TEMPLATE`].
    ///
    /// # Errors
    /// [`Error::ProjectExists`], or an I/O error writing the files.
    pub fn create_project(&self, id: &Slug, title: &str, links: Vec<String>) -> Result<Project> {
        let dir = self.root.join(id.as_str());
        if dir.exists() {
            return Err(Error::ProjectExists(id.to_string()));
        }
        let tasks = dir.join(TASKS_DIR);
        fs::create_dir_all(&tasks).map_err(Error::io(&tasks))?;
        let now = now();
        let meta = ProjectMeta {
            title: title.to_owned(),
            status: ProjectStatus::Planning,
            created: now,
            updated: None,
            links,
        };
        let file = dir.join(PROJECT_FILE);
        write_atomic(&file, &frontmatter::render(&file, &meta, PROJECT_TEMPLATE)?)?;
        append_line(
            &dir.join(LOG_FILE),
            &log_line(now, None, &format!("project created: {title}")),
        )?;
        self.project(id.as_str())
    }

    /// Appends a `todo` task, numbered after the last one, from
    /// [`TASK_TEMPLATE`].
    ///
    /// # Errors
    /// [`Error::ProjectNotFound`], [`Error::TaskExists`], or an I/O error.
    pub fn add_task(
        &self,
        project: &str,
        slug: &Slug,
        title: &str,
        links: Vec<String>,
    ) -> Result<Task> {
        let p = self.project(project)?;
        if p.tasks.iter().any(|t| t.slug == slug.as_str()) {
            return Err(Error::TaskExists(slug.to_string()));
        }
        let number = p
            .tasks
            .iter()
            .map(|t| t.number)
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        let id = format!("{number:02}-{slug}");
        let dir = p.dir.join(TASKS_DIR);
        fs::create_dir_all(&dir).map_err(Error::io(&dir))?;
        let path = dir.join(format!("{id}.md"));
        if path.exists() {
            return Err(Error::TaskExists(id));
        }
        let now = now();
        let meta = TaskMeta {
            title: title.to_owned(),
            status: TaskStatus::Todo,
            created: now,
            updated: None,
            links,
            sessions: Vec::new(),
        };
        write_atomic(&path, &frontmatter::render(&path, &meta, TASK_TEMPLATE)?)?;
        append_line(
            &p.log_file(),
            &log_line(now, Some(&id), &format!("task added: {title}")),
        )?;
        load_task(&path)?.ok_or(Error::TaskNotFound {
            project: project.to_owned(),
            task: id,
        })
    }

    /// Sets a task's status and returns `(task, previous status)`. Starting
    /// work on a `planning` project moves the project to `active`.
    ///
    /// # Errors
    /// [`Error::ProjectNotFound`], [`Error::TaskNotFound`], or the task
    /// file's read/parse/write error.
    pub fn set_task_status(
        &self,
        project: &str,
        reference: &str,
        status: TaskStatus,
    ) -> Result<(Task, TaskStatus)> {
        let p = self.project(project)?;
        let task = p.find_task(reference)?;
        let mut previous = status;
        let now = now();
        update_doc::<TaskMeta>(&task.path, |meta| {
            previous = meta.status;
            meta.status = status;
            meta.updated = Some(now);
        })?;
        append_line(
            &p.log_file(),
            &log_line(
                now,
                Some(&task.id),
                &format!("{} → {}", previous.as_str(), status.as_str()),
            ),
        )?;
        if matches!(status, TaskStatus::Working | TaskStatus::Waiting)
            && p.meta.status == ProjectStatus::Planning
        {
            self.set_project_status(project, ProjectStatus::Active)?;
        }
        let updated = load_task(&task.path)?.ok_or_else(|| Error::TaskNotFound {
            project: project.to_owned(),
            task: reference.to_owned(),
        })?;
        Ok((updated, previous))
    }

    /// Sets a project's status and returns the previous one.
    ///
    /// # Errors
    /// [`Error::ProjectNotFound`], or the project file's read/parse/write
    /// error.
    pub fn set_project_status(
        &self,
        project: &str,
        status: ProjectStatus,
    ) -> Result<ProjectStatus> {
        let p = self.project(project)?;
        let mut previous = status;
        let now = now();
        update_doc::<ProjectMeta>(&p.file(), |meta| {
            previous = meta.status;
            meta.status = status;
            meta.updated = Some(now);
        })?;
        append_line(
            &p.log_file(),
            &log_line(
                now,
                None,
                &format!("project {} → {}", previous.as_str(), status.as_str()),
            ),
        )?;
        Ok(previous)
    }

    /// Deletes a task file and returns what it held. Prefer `skipped` when
    /// the record is worth keeping.
    ///
    /// # Errors
    /// [`Error::ProjectNotFound`], [`Error::TaskNotFound`], or an I/O error.
    pub fn remove_task(&self, project: &str, reference: &str) -> Result<Task> {
        let p = self.project(project)?;
        let task = p.find_task(reference)?.clone();
        fs::remove_file(&task.path).map_err(Error::io(&task.path))?;
        append_line(
            &p.log_file(),
            &log_line(
                now(),
                Some(&task.id),
                &format!("task removed: {}", task.meta.title),
            ),
        )?;
        Ok(task)
    }

    /// Attaches a Claude Code session to a task, so the TUI's Session tab
    /// can show it. Returns false when it was already attached.
    ///
    /// # Errors
    /// [`Error::ProjectNotFound`], [`Error::TaskNotFound`], an invalid
    /// session id ([`Error::InvalidSlug`]), or the task file's
    /// read/parse/write error.
    pub fn attach_session(&self, project: &str, reference: &str, session: &str) -> Result<bool> {
        if !crate::transcript::is_session_id(session) {
            return Err(Error::InvalidSlug(session.to_owned()));
        }
        let p = self.project(project)?;
        let task = p.find_task(reference)?;
        if task.meta.sessions.iter().any(|s| s == session) {
            return Ok(false);
        }
        let now = now();
        update_doc::<TaskMeta>(&task.path, |meta| {
            meta.sessions.push(session.to_owned());
            meta.updated = Some(now);
        })?;
        let short = session.get(..8).unwrap_or(session);
        append_line(
            &p.log_file(),
            &log_line(now, Some(&task.id), &format!("attached session {short}")),
        )?;
        // The index the hooks use to find a session's projects quickly.
        let index = self.root.join(ATTACHED_DIR).join(session);
        if !self.attached_projects(session).contains(&p.id.to_string()) {
            if let Some(dir) = index.parent() {
                fs::create_dir_all(dir).map_err(Error::io(dir))?;
            }
            append_line(&index, p.id.as_str())?;
        }
        Ok(true)
    }

    /// The projects a session is attached to (from the index written by
    /// [`Store::attach_session`]); empty when none.
    #[must_use]
    pub fn attached_projects(&self, session: &str) -> Vec<String> {
        if !crate::transcript::is_session_id(session) {
            return Vec::new();
        }
        fs::read_to_string(self.root.join(ATTACHED_DIR).join(session))
            .map(|text| text.lines().map(str::to_owned).collect())
            .unwrap_or_default()
    }

    /// Attaches a link (ticket id, URL, `backlog:TASK-12`) to the project,
    /// or to one of its tasks. Attaching the same link twice is a no-op.
    ///
    /// # Errors
    /// [`Error::ProjectNotFound`], [`Error::TaskNotFound`], or the file's
    /// read/parse/write error.
    pub fn add_link(&self, project: &str, task: Option<&str>, link: &str) -> Result<()> {
        let p = self.project(project)?;
        let now = now();
        let push = |links: &mut Vec<String>| {
            if !links.iter().any(|l| l == link) {
                links.push(link.to_owned());
            }
        };
        let task_id = if let Some(reference) = task {
            let t = p.find_task(reference)?;
            update_doc::<TaskMeta>(&t.path, |meta| {
                push(&mut meta.links);
                meta.updated = Some(now);
            })?;
            Some(t.id.as_str())
        } else {
            update_doc::<ProjectMeta>(&p.file(), |meta| {
                push(&mut meta.links);
                meta.updated = Some(now);
            })?;
            None
        };
        append_line(
            &p.log_file(),
            &log_line(now, task_id, &format!("linked {link}")),
        )
    }

    /// Appends a narration line to the project's log, optionally tagged with
    /// a task. Newlines in `message` are flattened to spaces.
    ///
    /// # Errors
    /// [`Error::ProjectNotFound`], [`Error::TaskNotFound`], or an I/O error.
    pub fn append_log(&self, project: &str, task: Option<&str>, message: &str) -> Result<()> {
        let p = self.project(project)?;
        let task = task.map(|reference| p.find_task(reference)).transpose()?;
        append_line(
            &p.log_file(),
            &log_line(now(), task.map(|t| t.id.as_str()), message),
        )?;
        // Narrating a paused task means someone is on it again.
        if let Some(t) = task.filter(|t| t.meta.status == TaskStatus::Paused) {
            self.set_task_status(project, &t.id, TaskStatus::Working)?;
        }
        Ok(())
    }

    /// Pauses every `working` task whose latest activity (as `activity`
    /// reports it) is at least `quiet_for` before `now`, logging why.
    /// Returns what it paused.
    ///
    /// # Errors
    /// [`Error::Io`] if the store cannot be listed or a task file written.
    pub fn pause_quiet_tasks(
        &self,
        quiet_for: std::time::Duration,
        activity: impl Fn(&Project, &Task) -> Option<SystemTime>,
        now: SystemTime,
    ) -> Result<Vec<AutoPaused>> {
        let mut paused = Vec::new();
        for project in self.list()?.projects {
            for task in project
                .tasks
                .iter()
                .filter(|t| t.meta.status == TaskStatus::Working)
            {
                let Some(last) = activity(&project, task) else {
                    continue;
                };
                let quiet = now.duration_since(last).unwrap_or_default();
                if quiet < quiet_for {
                    continue;
                }
                self.set_task_status(project.id.as_str(), &task.id, TaskStatus::Paused)?;
                let minutes = quiet.as_secs() / 60;
                let label = if minutes == 0 {
                    format!("{}s", quiet.as_secs())
                } else if minutes < 60 {
                    format!("{minutes}m")
                } else {
                    format!("{}h{:02}m", minutes / 60, minutes % 60)
                };
                append_line(
                    &project.log_file(),
                    &log_line(
                        now_from(now),
                        Some(&task.id),
                        &format!("auto-paused: quiet {label} (no activity from the agent)"),
                    ),
                )?;
                paused.push(AutoPaused {
                    project_id: project.id.to_string(),
                    project_title: project.meta.title.clone(),
                    task: format!("{} · {}", task.id, task.meta.title),
                    task_id: task.id.clone(),
                    task_title: task.meta.title.clone(),
                    quiet: label,
                });
            }
        }
        Ok(paused)
    }
}

/// A task [`Store::pause_quiet_tasks`] paused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoPaused {
    /// The project id.
    pub project_id: String,
    /// The project title.
    pub project_title: String,
    /// `NN-slug · title`.
    pub task: String,
    /// The task id (`NN-slug`).
    pub task_id: String,
    /// The task title.
    pub task_title: String,
    /// How long it had been quiet, e.g. `34m`.
    pub quiet: String,
}
