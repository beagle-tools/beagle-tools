import { useEffect, useState, type ReactElement } from "react";

import { errorMessage, ipc } from "../lib/ipc";
import type { ChainView, Run } from "../lib/types";
import { useStore } from "../state/store";
import { ChainBlock } from "./JobPage";
import { StepPane } from "./StepPane";

// A job's Runs tab, for every job at once: each run behind its partition
// (naming its job), newest first, its steps beneath, and the step you
// click open beside the list. Re-read on every store change, so runs
// arrive while you watch. "Running only" is the "what is this machine
// doing right now" view.
export function AllRunsPage(): ReactElement {
  const forest = useStore((s) => s.forest);
  const selectedId = useStore((s) => s.selectedId);
  const select = useStore((s) => s.select);

  const [chains, setChains] = useState<ChainView[] | null>(null);
  const [runningOnly, setRunningOnly] = useState(false);
  const [showHidden, setShowHidden] = useState(false);

  useEffect(() => {
    let cancelled = false;
    ipc
      .allChains(showHidden, runningOnly)
      .then((c) => {
        if (!cancelled) setChains(c);
      })
      .catch((e: unknown) => {
        if (!cancelled) useStore.setState({ error: errorMessage(e) });
      });
    return () => {
      cancelled = true;
    };
  }, [forest, showHidden, runningOnly]);

  const steps: Run[] = chains?.flatMap((c) => c.steps.map((s) => s.run)) ?? [];
  const selected = steps.find((r) => r.id === selectedId) ?? null;

  return (
    <div className="page job">
      <div className="panel">
        <div className="panel-head">
          <h2>All runs</h2>
          <span className="muted small">
            {chains ? `${String(chains.length)} run${chains.length === 1 ? "" : "s"}` : ""}
          </span>
          <div className="seg">
            <button
              className={!runningOnly ? "on" : ""}
              onClick={() => {
                setRunningOnly(false);
              }}
            >
              All
            </button>
            <button
              className={runningOnly ? "on" : ""}
              onClick={() => {
                setRunningOnly(true);
              }}
            >
              Running
            </button>
          </div>
          <div className="panel-actions">
            <label className="muted small showhidden" title="Projects hidden in ui.yaml">
              <input
                type="checkbox"
                checked={showHidden}
                onChange={(e) => {
                  setShowHidden(e.target.checked);
                }}
              />{" "}
              Show hidden
            </label>
          </div>
        </div>

        <div className="panel-body">
          {!chains ? (
            <div className="empty muted">Loading…</div>
          ) : chains.length === 0 ? (
            <div className="empty muted">
              {runningOnly ? "Nothing is running right now." : "No runs yet."}
            </div>
          ) : (
            <div className="runsplit">
              <div className="runlist-col">
                {chains.map((chain) => (
                  <ChainBlock
                    key={chain.root.id}
                    chain={chain}
                    selectedId={selectedId}
                    onPick={(id) => void select(id)}
                    showJob
                  />
                ))}
              </div>
              <div className="steppane-col">
                {selected ? (
                  <StepPane run={selected} />
                ) : (
                  <div className="empty muted">Click a step to see its log.</div>
                )}
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
