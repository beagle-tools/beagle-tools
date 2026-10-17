import type { ReactElement } from "react";
import { cost, duration } from "../lib/format";
import { NODE_H, NODE_W, type GraphNode } from "../lib/graph";
import { isActive } from "../lib/types";
import { useStore } from "../state/store";

// One run in the graph. Carries its own actions: a failed stage can be
// retried from here and a live one stopped, because the graph is where
// you are when you notice either — making you go back to a list to act
// on what you are looking at is the wrong shape.
export function RunNode({ node }: { node: GraphNode }): ReactElement {
  const selectedId = useStore((s) => s.selectedId);
  const select = useStore((s) => s.select);
  const kill = useStore((s) => s.kill);
  const retry = useStore((s) => s.retry);

  const run = node.row.run;
  const active = isActive(run.state);
  const broken = run.state === "failed" || run.state === "timedout" || run.state === "killed";

  return (
    <g
      transform={`translate(${String(node.x)}, ${String(node.y)})`}
      className={`gnode ${run.state}${run.id === selectedId ? " selected" : ""}`}
    >
      <rect
        width={NODE_W}
        height={NODE_H}
        rx={10}
        onClick={() => void select(run.id)}
        role="button"
        tabIndex={0}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") void select(run.id);
        }}
      />
      <circle cx={17} cy={23} r={5} className="dot" />
      <text x={32} y={27} className="gname">
        {run.agent}
      </text>
      <text x={32} y={44} className="gmeta">
        {duration(run.started_at, run.finished_at)}
        {run.cost_usd !== null ? ` · ${cost(run.cost_usd)}` : ""}
        {run.exit_code !== null && run.exit_code !== 0 ? ` · exit ${String(run.exit_code)}` : ""}
      </text>

      {(active || broken) && (
        <g
          className="chip"
          transform={`translate(${String(NODE_W - 62)}, ${String(NODE_H - 22)})`}
          onClick={() => (active ? void kill(run.id) : void retry(run.id))}
          role="button"
          tabIndex={0}
          onKeyDown={(e) => {
            if (e.key === "Enter") void (active ? kill(run.id) : retry(run.id));
          }}
        >
          <rect width={52} height={16} rx={8} />
          <text x={26} y={12} textAnchor="middle">
            {active ? "stop" : "retry"}
          </text>
        </g>
      )}
    </g>
  );
}
