import { useCallback, useEffect, useState, type ReactElement } from "react";

import { DagPage } from "./components/DagPage";
import { JobLauncher } from "./components/JobLauncher";
import { JobPage } from "./components/JobPage";
import { JobsPage } from "./components/JobsPage";
import { useShortcuts } from "./lib/shortcuts";
import type { PipelineSummary } from "./lib/types";
import { useStore } from "./state/store";

// Two places to be, not four. You open on the JOBS you can start; one
// of them opens into its graph, its runs, and a step's log. Activity is
// the cross-job view — every chain at once, for "what is this machine
// doing right now".
//
// The old Runs page is gone: a flat list of every run mixed entry
// points with steps, which is precisely the distinction that makes this
// readable.
type Page = "jobs" | "activity";

export default function App(): ReactElement {
  const init = useStore((s) => s.init);
  const error = useStore((s) => s.error);
  const dismissError = useStore((s) => s.dismissError);
  const selectedId = useStore((s) => s.selectedId);
  const setDagFocus = useStore((s) => s.setDagFocus);
  const dagFocus = useStore((s) => s.dagFocus);
  const kill = useStore((s) => s.kill);
  const runs = useStore((s) => s.runs);
  const pipelines = useStore((s) => s.pipelines);

  const [page, setPage] = useState<Page>("jobs");
  const [openJob, setOpenJob] = useState<PipelineSummary | null>(null);
  const [launcherOpen, setLauncherOpen] = useState(false);

  useEffect(() => {
    void init();
  }, [init]);

  // Keep the open job's status live — it is what tells you the run you
  // are watching has moved on to the next step.
  const current = openJob
    ? (pipelines.find((p) => p.project === openJob.project && p.name === openJob.name) ?? openJob)
    : null;

  useShortcuts({
    onDag: useCallback(() => {
      setPage((p) => (p === "activity" ? "jobs" : "activity"));
    }, []),
    onPinChain: useCallback(() => {
      if (!selectedId) return;
      void setDagFocus(selectedId);
      setPage("activity");
    }, [selectedId, setDagFocus]),
    onEscape: useCallback(() => {
      if (dagFocus) void setDagFocus(null);
      else if (openJob) setOpenJob(null);
      else setPage("jobs");
    }, [dagFocus, openJob, setDagFocus]),
    onKill: useCallback(() => {
      const run = runs.find((r) => r.id === selectedId);
      if (run && (run.state === "running" || run.state === "pending")) void kill(run.id);
    }, [runs, selectedId, kill]),
  });

  const anyRunning = pipelines.filter((p) => p.active_runs > 0);

  return (
    <div className="app">
      <nav className="topbar">
        <strong className="brand">Agentdesk</strong>
        <div className="seg nav">
          <button
            className={page === "jobs" ? "on" : ""}
            onClick={() => {
              setPage("jobs");
              setOpenJob(null);
            }}
          >
            Jobs
          </button>
          <button
            className={page === "activity" ? "on" : ""}
            onClick={() => {
              setPage("activity");
            }}
          >
            Activity
          </button>
        </div>
        {/* Always visible, on every page: is anything running at all. */}
        <span className="topstatus muted small">
          {anyRunning.length === 0
            ? "idle"
            : `${String(anyRunning.length)} running · ${anyRunning
                .flatMap((p) => p.active_steps)
                .join(", ")}`}
        </span>
        <button
          className="primary"
          onClick={() => {
            setLauncherOpen(true);
          }}
        >
          Run job
        </button>
      </nav>

      {error && (
        <div className="errorbar">
          <span>{error}</span>
          <button onClick={dismissError}>Dismiss</button>
        </div>
      )}

      {page === "activity" ? (
        <DagPage />
      ) : current ? (
        <JobPage
          job={current}
          onBack={() => {
            setOpenJob(null);
          }}
        />
      ) : (
        <JobsPage onOpen={setOpenJob} />
      )}

      {launcherOpen && (
        <JobLauncher
          onClose={() => {
            setLauncherOpen(false);
          }}
        />
      )}
    </div>
  );
}
