//! `agentctl errors` — what broke since a point in time, deduplicated.
//!
//! The deterministic first step of the pipeline resolver: it reads the
//! store (see `agent_core::errscan`), groups repeats, and writes a report a
//! model can act on — every group with its job, step, manifest, purpose
//! file, count, and the runs to look at. Never further back than
//! [`MAX_HOURS`].

use std::fmt::Write as _;
use std::path::Path;

use agent_core::errscan::{scan, ErrorGroup, ErrorKind};
use agent_core::state::{RunFilter, Store};
use agent_core::types::RunState;
use anyhow::{bail, Context, Result};
use chrono::{DateTime, Duration, Utc};

/// The furthest back a scan may look, whatever it is asked.
pub const MAX_HOURS: u32 = 48;

/// Where the window starts and why — for the report header.
#[derive(Debug)]
pub struct Window {
    pub since: DateTime<Utc>,
    pub until: DateTime<Utc>,
    pub basis: String,
}

/// Work out the window: from `since`, or from the start of `since_last`'s
/// previous successful run (not this one), never more than `max_hours`
/// (capped at [`MAX_HOURS`]) back.
pub fn window(
    store: &Store,
    since: Option<&str>,
    since_last: Option<&str>,
    max_hours: u32,
    now: DateTime<Utc>,
) -> Result<Window> {
    let hours = max_hours.min(MAX_HOURS);
    let floor = now - Duration::hours(i64::from(hours));
    let (start, basis) = if let Some(text) = since {
        let t = DateTime::parse_from_rfc3339(text)
            .with_context(|| format!("--since must be RFC 3339, got {text:?}"))?
            .with_timezone(&Utc);
        (Some(t), format!("--since {text}"))
    } else if let Some(target) = since_last {
        let Some((project, agent)) = target.split_once('/') else {
            bail!("--since-last takes <project>/<agent>, got {target:?}");
        };
        let me = std::env::var("AGENTD_RUN_ID").ok();
        let previous = store
            .list_runs(&RunFilter {
                project: Some(project.to_owned()),
                agent: Some(agent.to_owned()),
                states: Some(vec![RunState::Succeeded]),
                limit: Some(20),
            })?
            .into_iter()
            .find(|r| Some(&r.id) != me.as_ref());
        match previous.and_then(|r| r.started_at.map(|t| (t, r.id))) {
            Some((t, id)) => (
                Some(t),
                format!("since the last successful {target} run, {id}"),
            ),
            None => (None, format!("no earlier successful {target} run")),
        }
    } else {
        (None, "no start given".to_owned())
    };
    Ok(match start {
        Some(t) if t > floor => Window {
            since: t,
            until: now,
            basis,
        },
        _ => Window {
            since: floor,
            until: now,
            basis: format!("{basis} — capped at {hours}h back"),
        },
    })
}

/// `agentctl errors`: print the report, or with `write` save `errors.md`
/// and `errors.json` there and print only the number of groups.
pub fn errors(
    store: &Store,
    since: Option<&str>,
    since_last: Option<&str>,
    max_hours: u32,
    json: bool,
    write: Option<&Path>,
) -> Result<()> {
    let w = window(store, since, since_last, max_hours, Utc::now())?;
    let data_root = agent_core::paths::data_dir()?;
    let groups = scan(store, w.since, w.until, &data_root)?;
    let as_json = serde_json::to_string_pretty(&serde_json::json!({
        "since": w.since, "until": w.until, "basis": w.basis, "groups": groups,
    }))?;
    if let Some(dir) = write {
        std::fs::create_dir_all(dir)?;
        std::fs::write(dir.join("errors.json"), &as_json)?;
        std::fs::write(dir.join("errors.md"), render(&w, &groups))?;
        println!("{}", groups.len());
    } else if json {
        println!("{as_json}");
    } else {
        print!("{}", render(&w, &groups));
    }
    Ok(())
}

/// The report as markdown, one section per group, newest first.
pub fn render(w: &Window, groups: &[ErrorGroup]) -> String {
    let mut out = String::new();
    let total: usize = groups.iter().map(|g| g.count).sum();
    let _ = writeln!(out, "# Pipeline errors\n");
    let _ = writeln!(
        out,
        "Window: {} → {} ({}).\n{} distinct error(s), {} occurrence(s).\n",
        w.since.to_rfc3339(),
        w.until.to_rfc3339(),
        w.basis,
        groups.len(),
        total
    );
    for (i, g) in groups.iter().enumerate() {
        let kind = match g.kind {
            ErrorKind::Failed => "run failed",
            ErrorKind::TimedOut => "run timed out",
            ErrorKind::Logged => "ERROR logged (run carried on)",
        };
        let _ = writeln!(out, "## {}. {} — {} × {}\n", i + 1, g.step, g.count, kind);
        let _ = writeln!(out, "- job: `{}`", g.job);
        let path = |p: &Option<std::path::PathBuf>| {
            p.as_ref()
                .map_or_else(|| "—".to_owned(), |p| format!("`{}`", p.display()))
        };
        let _ = writeln!(out, "- job purpose: {}", path(&g.purpose));
        let _ = writeln!(out, "- step manifest: {}", path(&g.manifest));
        let _ = writeln!(
            out,
            "- seen: {} → {}",
            g.first_seen.to_rfc3339(),
            g.last_seen.to_rfc3339()
        );
        let _ = writeln!(out, "\n```text\n{}\n```\n", g.message);
        let _ = writeln!(out, "Runs (newest first):\n");
        for r in &g.runs {
            let _ = writeln!(
                out,
                "- `{}` {} {} · params {} · log {} · data {}",
                r.id,
                r.state,
                r.finished_at.map(|t| t.to_rfc3339()).unwrap_or_default(),
                r.params.as_deref().unwrap_or("—"),
                path(&r.log_path),
                path(&r.data_dir),
            );
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
// Tests should fail loudly on a broken fixture; unwrapping is the clearest way.
#[allow(clippy::unwrap_used)]
mod tests {
    use agent_core::types::Trigger;

    use super::*;

    #[test]
    fn the_window_never_reaches_past_48_hours() {
        let dir = std::env::temp_dir().join(format!("agentctl-errors-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = Store::open(&dir.join("state.db")).unwrap();
        let now = Utc::now();

        let w = window(&store, Some("2020-01-01T00:00:00Z"), None, 500, now).unwrap();
        assert_eq!(w.since, now - Duration::hours(48));
        assert!(w.basis.contains("capped at 48h"));

        // No earlier resolver run: the full (capped) window.
        let w = window(&store, None, Some("agentd/job-doctor"), 48, now).unwrap();
        assert_eq!(w.since, now - Duration::hours(48));

        // An earlier successful run: from its start.
        let prev = store
            .create_pending_run(
                "agentd",
                "job-doctor",
                Trigger::Cron,
                None,
                None,
                None,
            )
            .unwrap();
        store
            .mark_running(&prev.id, "s", Path::new("/x.log"), Path::new("/"))
            .unwrap();
        store
            .finish_run(&prev.id, RunState::Succeeded, Some(0))
            .unwrap();
        let w = window(&store, None, Some("agentd/job-doctor"), 48, now).unwrap();
        assert!(w.since > now - Duration::hours(1));
        assert!(w.basis.contains(&prev.id));

        let _ = std::fs::remove_dir_all(dir);
    }
}
