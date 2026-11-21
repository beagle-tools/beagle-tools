import { useCallback, useMemo, useRef, useState, type ReactElement } from "react";

import { age } from "../lib/format";
import { fuzzyMatch, highlightRuns, type FuzzyMatch } from "../lib/fuzzy";
import { treePrefixes } from "../lib/tree";
import type { PipelineSummary } from "../lib/types";
import { useStore } from "../state/store";
import { JobMenu } from "./ConfirmRun";
import { RunningFilter, ShowHidden, useJobsKeys } from "./JobsControls";

// The landing page, laid out like agenttop's: a section per project, each
// job a row with its whole flow drawn beneath it. The flow is the
// picture of the job, not a list of things to click — you click the job.
// A job's best match for the search: its name first, then any of its
// steps (find a job by the step you remember), then its project.
interface Ranked {
  job: PipelineSummary;
  score: number;
  name: FuzzyMatch | null;
  steps: Set<string>;
}

function rank(query: string, job: PipelineSummary): Ranked | null {
  const name = fuzzyMatch(query, job.name);
  const steps = new Set<string>();
  let stepBest = -Infinity;
  for (const node of job.flow.slice(1)) {
    const m = fuzzyMatch(query, node.agent);
    if (m) {
      steps.add(node.agent);
      stepBest = Math.max(stepBest, m.score);
    }
  }
  const project = fuzzyMatch(query, job.project);
  const scores = [
    name ? name.score : -Infinity,
    steps.size ? stepBest * 0.8 : -Infinity,
    project ? project.score * 0.5 : -Infinity,
  ];
  const score = Math.max(...scores);
  if (score === -Infinity) return null;
  return { job, score, name, steps };
}

export function JobsPage({ onOpen }: { onOpen: (p: PipelineSummary) => void }): ReactElement {
  const pipelines = useStore((s) => s.pipelines);
  const loading = useStore((s) => s.loading);
  const [query, setQuery] = useState("");
  // The All | Running toggle (`a` / `r`; agenttop's `a` / `f`).
  const [onlyRunning, setOnlyRunning] = useState(false);
  // ui.yaml's hidden projects (demo): off the page until this is ticked.
  const [showHidden, setShowHidden] = useState(false);
  const input = useRef<HTMLInputElement>(null);

  const toggleHidden = useCallback(() => {
    setShowHidden((v) => !v);
  }, []);
  useJobsKeys(input, setOnlyRunning, toggleHidden);

  const searching = query.trim() !== "";
  // What the list shows: every job, or only those with a run live.
  const shown = useMemo(
    () => (showHidden ? pipelines : pipelines.filter((p) => !p.hidden)),
    [pipelines, showHidden],
  );
  const hiddenCount = pipelines.length - pipelines.filter((p) => !p.hidden).length;
  const visible = useMemo(
    () => (onlyRunning ? shown.filter((p) => p.active_runs > 0) : shown),
    [shown, onlyRunning],
  );
  const ranked = useMemo(() => {
    if (!searching) return null;
    return visible
      .map((job) => rank(query, job))
      .filter((r): r is Ranked => r !== null)
      .sort((a, b) => b.score - a.score);
  }, [visible, query, searching]);

  if (loading) return <div className="empty muted">Loading…</div>;
  if (pipelines.length === 0) {
    return <div className="empty muted">No jobs. Add a manifest and run `beagle agent apply`.</div>;
  }

  const running = shown.filter((p) => p.active_runs > 0);
  // The backend already sorts by project then name; group in that order.
  const projects = [...new Set(visible.map((p) => p.project))];

  return (
    <div className="page jobs">
      <div className="panel">
        <div className="panel-head">
          <h2>Jobs</h2>
          <span className="muted small">
            Each job is something you start. Click one to see its shape, its runs, and each
            step&apos;s log.
          </span>
          <span className="panel-actions muted small">
            {running.length === 0
              ? "nothing running"
              : running
                  .map((p) => `${p.name} at ${p.active_steps.join(", ") || "starting"}`)
                  .join(" · ")}
          </span>
        </div>

        <div className="jobsearch">
          <input
            ref={input}
            type="search"
            placeholder="Fuzzy-search jobs, steps and projects   ( / )"
            value={query}
            spellCheck={false}
            onChange={(e) => {
              setQuery(e.target.value);
            }}
            onKeyDown={(e) => {
              if (e.key === "Escape") {
                setQuery("");
                input.current?.blur();
              } else if (e.key === "Enter" && ranked?.[0]) {
                onOpen(ranked[0].job);
              }
            }}
          />
          <RunningFilter
            onlyRunning={onlyRunning}
            runningCount={running.length}
            setOnlyRunning={setOnlyRunning}
          />
          <ShowHidden count={hiddenCount} checked={showHidden} onChange={setShowHidden} />
          {ranked && (
            <span className="muted small">
              {ranked.length === 0
                ? "no match"
                : `${String(ranked.length)} of ${String(visible.length)} · enter opens the first`}
            </span>
          )}
        </div>

        <div className="panel-body">
          {onlyRunning && visible.length === 0 && (
            <div className="empty muted">Nothing running right now — All shows every job.</div>
          )}
          {ranked
            ? ranked.map((r) => (
                <JobBlock
                  key={`${r.job.project}/${r.job.name}`}
                  job={r.job}
                  onOpen={onOpen}
                  match={r}
                />
              ))
            : projects.map((project) => (
                <section key={project} className="project">
                  <h3 className="project-name">{project}</h3>
                  {visible
                    .filter((p) => p.project === project)
                    .map((job) => (
                      <JobBlock key={job.name} job={job} onOpen={onOpen} />
                    ))}
                </section>
              ))}
        </div>
      </div>
    </div>
  );
}

function JobBlock({
  job,
  onOpen,
  match,
}: {
  job: PipelineSummary;
  onOpen: (p: PipelineSummary) => void;
  /** Set while searching: highlight what matched. */
  match?: Ranked;
}) {
  const running = job.active_runs > 0;
  const last = job.last;
  const prefixes = treePrefixes(job.flow.map((n) => n.depth));

  return (
    <div
      className={`jobblock${running ? " running" : ""}`}
      onClick={() => {
        onOpen(job);
      }}
    >
      <div className="jobline">
        <span className={`dot ${running ? "running" : (last?.state ?? "none")}`} aria-hidden />
        <span className="job-name">
          {match?.name
            ? highlightRuns(job.name, match.name.positions).map((run, i) =>
                run.hit ? <mark key={i}>{run.text}</mark> : <span key={i}>{run.text}</span>,
              )
            : job.name}
        </span>
        {match && <span className="muted small">{job.project}</span>}
        <span className="muted small">{job.runner}</span>
        <span className="muted small">
          {job.schedule ? <code>cron {job.schedule}</code> : "manual"}
          {!job.enabled && " · disabled"}
        </span>
        <span className="jobline-state">
          {running ? (
            <span className="pill running">
              running · {job.active_steps.join(", ") || "starting"}
              {job.active_runs > 1 ? ` ×${String(job.active_runs)}` : ""}
            </span>
          ) : last ? (
            <>
              <span className={`badge ${last.state}`}>{last.state}</span>
              <span className="muted small">{age(last.created_at)} ago</span>
            </>
          ) : (
            <span className="muted small">never run</span>
          )}
          <JobMenu project={job.project} name={job.name} />
        </span>
      </div>

      {job.flow.length > 1 ? (
        <pre className="flowtree">
          {job.flow.slice(1).map((node, i) => {
            const live = job.active_steps.includes(node.agent);
            const hit = match?.steps.has(node.agent);
            const cls = [live && "live", hit && "hit"].filter(Boolean).join(" ");
            return (
              <div key={`${String(i)}-${node.agent}`} className={cls || undefined}>
                <span className="guide">{prefixes[i + 1]}</span>
                {node.agent}
                {live && <span className="livetag"> ● running</span>}
              </div>
            );
          })}
        </pre>
      ) : (
        !last && <div className="muted small flowhint">its steps appear once it has run</div>
      )}
    </div>
  );
}
