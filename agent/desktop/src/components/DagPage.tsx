import { useState, type ReactElement } from "react";

import { useStore } from "../state/store";
import { FlowList } from "./FlowList";
import { RunGraph } from "./RunGraph";
import { Topology } from "./Topology";

type Mode = "graph" | "flows" | "topology";

const BLURB: Record<Mode, string> = {
  graph: "The chain as it ran — a column per depth, the root carrying the total.",
  flows: "Every chain as indented rows. Best for scanning a lot of them, and for run ids.",
  topology: "What the manifests declare, whether or not it has ever run.",
};

// Its own page, full width. The DAG answers "what is the shape of this
// work and what did it cost" — a different question from "what is this
// one run doing", and it wants the whole window to answer it.
export function DagPage(): ReactElement {
  const forest = useStore((s) => s.forest);
  const dagFocus = useStore((s) => s.dagFocus);
  const setDagFocus = useStore((s) => s.setDagFocus);
  const selectedId = useStore((s) => s.selectedId);
  const [mode, setMode] = useState<Mode>("graph");

  const chains = forest.filter((r) => r.depth === 0).length;

  return (
    <div className="page dag">
      <div className="panel">
        <div className="panel-head">
          <div className="seg">
            {(["graph", "flows", "topology"] as Mode[]).map((m) => (
              <button
                key={m}
                className={mode === m ? "on" : ""}
                onClick={() => {
                  setMode(m);
                }}
              >
                {m.charAt(0).toUpperCase() + m.slice(1)}
              </button>
            ))}
          </div>

          <p className="blurb muted small">{BLURB[mode]}</p>

          {mode !== "topology" && (
            <div className="panel-actions">
              {dagFocus ? (
                <>
                  <span className="pill">one chain</span>
                  <button onClick={() => void setDagFocus(null)}>Show all</button>
                </>
              ) : (
                <>
                  <span className="muted small">
                    {chains} chain{chains === 1 ? "" : "s"}
                  </span>
                  <button
                    disabled={!selectedId}
                    title="D — narrow to the selected run's chain"
                    onClick={() => void setDagFocus(selectedId)}
                  >
                    Pin this chain
                  </button>
                </>
              )}
            </div>
          )}
        </div>

        <div className="panel-body">
          {mode === "graph" && <RunGraph rows={forest} />}
          {mode === "flows" && <FlowList rows={forest} />}
          {mode === "topology" && <Topology />}
        </div>
      </div>
    </div>
  );
}
