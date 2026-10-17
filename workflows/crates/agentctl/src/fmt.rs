//! Aligned table rendering and human-readable age/duration formatting.

use agent_core::types::{Run, RunState};
use beagle_utils::duration::{age_single_unit, human_duration};
use chrono::{DateTime, Utc};

/// A hand-rolled, kubectl-style aligned table: UPPERCASE headers supplied by
/// the caller, two-space gutters, no borders, no color.
#[derive(Debug)]
pub struct Table {
    headers: Vec<&'static str>,
    rows: Vec<Vec<String>>,
}

impl Table {
    pub fn new(headers: &[&'static str]) -> Self {
        Self {
            headers: headers.to_vec(),
            rows: Vec::new(),
        }
    }

    pub fn row(&mut self, cells: Vec<String>) {
        self.rows.push(cells);
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Render to stdout with per-column widths computed over all cells.
    pub fn print(&self) {
        let mut widths: Vec<usize> = self.headers.iter().map(|h| h.len()).collect();
        for row in &self.rows {
            for (width, cell) in widths.iter_mut().zip(row) {
                *width = (*width).max(cell.len());
            }
        }
        println!("{}", render_row(self.headers.iter().copied(), &widths));
        for row in &self.rows {
            println!("{}", render_row(row.iter().map(String::as_str), &widths));
        }
    }
}

fn render_row<'a>(cells: impl Iterator<Item = &'a str>, widths: &[usize]) -> String {
    let mut line = String::new();
    for (cell, width) in cells.zip(widths) {
        if !line.is_empty() {
            line.push_str("  ");
        }
        line.push_str(cell);
        line.push_str(&" ".repeat(width.saturating_sub(cell.len())));
    }
    line.trim_end().to_owned()
}

/// `$1.04`, or `-` when the runner reported no price (codex) or the run has
/// not finished. Four decimals below a cent so a cheap run is not just `$0.00`.
pub fn cost(cost_usd: Option<f64>) -> String {
    match cost_usd {
        Some(c) if c >= 0.01 => format!("${c:.2}"),
        Some(c) if c > 0.0 => format!("${c:.4}"),
        Some(_) => "$0".to_owned(),
        None => "-".to_owned(),
    }
}

/// Age of a timestamp relative to `now`.
pub fn age(from: DateTime<Utc>, now: DateTime<Utc>) -> String {
    age_single_unit((now - from).num_seconds())
}

/// Wall-clock duration of a run: started to finished, started to `now` while
/// still running, "-" when it never started.
pub fn run_duration(run: &Run, now: DateTime<Utc>) -> String {
    match (run.started_at, run.finished_at) {
        (Some(start), Some(end)) => human_duration((end - start).num_seconds()),
        (Some(start), None) if run.state == RunState::Running => {
            human_duration((now - start).num_seconds())
        }
        _ => "-".to_owned(),
    }
}

/// Collapse all whitespace runs (including newlines) to single spaces and
/// truncate to `max` characters, appending "..." when cut.
pub fn single_line_truncated(text: &str, max: usize) -> String {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= max {
        collapsed
    } else {
        let mut out: String = collapsed.chars().take(max).collect();
        out.push_str("...");
        out
    }
}

#[cfg(test)]
// Tests should fail loudly on a broken fixture; unwrapping is the clearest way.
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use agent_core::types::Trigger;
    use chrono::TimeDelta;

    #[test]
    fn truncation_collapses_and_cuts() {
        assert_eq!(single_line_truncated("a\nb\t c", 120), "a b c");
        assert_eq!(single_line_truncated(&"x".repeat(10), 4), "xxxx...");
        assert_eq!(single_line_truncated("short", 120), "short");
    }

    fn run_at(now: DateTime<Utc>, started: bool, finished: bool, state: RunState) -> Run {
        Run {
            id: "x".into(),
            project: "p".into(),
            agent: "a".into(),
            state,
            trigger: Trigger::Manual,
            parent_run_id: None,
            params: None,
            tmux_session: None,
            log_path: None,
            workdir: None,
            model: None,
            created_at: now - TimeDelta::seconds(120),
            started_at: started.then(|| now - TimeDelta::seconds(90)),
            finished_at: finished.then(|| now - TimeDelta::seconds(10)),
            exit_code: None,
            cost_usd: None,
            num_turns: None,
            usage_json: None,
        }
    }

    #[test]
    fn run_durations() {
        let now = Utc::now();
        assert_eq!(
            run_duration(&run_at(now, true, true, RunState::Succeeded), now),
            "1m"
        );
        assert_eq!(
            run_duration(&run_at(now, false, false, RunState::Pending), now),
            "-"
        );
        assert_eq!(
            run_duration(&run_at(now, true, false, RunState::Running), now),
            "1m"
        );
        // Started but unfinished in a terminal state (crashed daemon): no duration.
        assert_eq!(
            run_duration(&run_at(now, true, false, RunState::Failed), now),
            "-"
        );
    }

    #[test]
    fn tables_align_columns() {
        let mut table = Table::new(&["NAME", "AGE"]);
        table.row(vec!["longer-name".into(), "3m".into()]);
        assert!(!table.is_empty());
        // Rendering itself is exercised via print(); check width math directly.
        let widths = vec![11, 3];
        assert_eq!(
            render_row(["NAME", "AGE"].into_iter(), &widths),
            "NAME         AGE"
        );
        assert_eq!(
            render_row(["longer-name", "3m"].into_iter(), &widths),
            "longer-name  3m"
        );
    }
}
