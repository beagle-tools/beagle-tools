import type { ReactElement } from "react";
import { age } from "../lib/format";
import { treePrefixes } from "../lib/tree";
import type { PipelineSummary } from "../lib/types";
import { useStore } from "../state/store";

// The landing page, laid out like agenttop's: a section per project, each
// job a row with its whole flow drawn beneath it. The flow is the
// picture of the job, not a list of things to click — you click the job.
export function JobsPage({ onOpen }: { onOpen: (p: PipelineSummary) => void }): ReactElement {
  const pipelines = useStore((s) => s.pipelines);
  const loading = useStore((s) => s.loading);

  if (loading) return <div className="empty muted">Loading…</div>;
  if (pipelines.length === 0) {
    return <div className="empty muted">No jobs. Add a manifest and run `agentctl apply`.</div>;
  }

  const running = pipelines.filter((p) => p.active_runs > 0);
  // The backend already sorts by project then name; group in that order.
  const projects = [...new Set(pipelines.map((p) => p.project))];

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

        <div className="panel-body">
          {projects.map((project) => (
            <section key={project} className="project">
              <h3 className="project-name">{project}</h3>
              {pipelines
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

function JobBlock({ job, onOpen }: { job: PipelineSummary; onOpen: (p: PipelineSummary) => void }) {
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
        <span className="job-name">{job.name}</span>
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
        </span>
      </div>

      {job.flow.length > 1 ? (
        <pre className="flowtree">
          {job.flow.slice(1).map((node, i) => {
            const live = job.active_steps.includes(node.agent);
            return (
              <div key={`${String(i)}-${node.agent}`} className={live ? "live" : undefined}>
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
