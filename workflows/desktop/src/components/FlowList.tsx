import type { ReactElement } from "react";
import { age, cost } from "../lib/format";
import { isActive } from "../lib/types";
import type { ForestRow } from "../lib/types";
import { useStore } from "../state/store";
import { StateBadge } from "./RunList";

// agenttop's FLOWS pane: the lineage forest as indented text. The graph
// is better for shape; this is better for scanning a lot of chains, and
// for seeing ids.
export function FlowList({ rows }: { rows: ForestRow[] }): ReactElement {
  const selectedId = useStore((s) => s.selectedId);
  const select = useStore((s) => s.select);
  const kill = useStore((s) => s.kill);
  const retry = useStore((s) => s.retry);

  if (rows.length === 0) return <div className="empty muted">No runs yet.</div>;

  return (
    <ul className="flows">
      {rows.map(({ run, depth, chain_cost }) => (
        <li
          key={run.id}
          className={`${depth === 0 ? "root " : ""}${run.id === selectedId ? "selected" : ""}`}
          style={{ paddingLeft: `${String(depth * 22 + 10)}px` }}
          onClick={() => void select(run.id)}
        >
          <span className="muted">{depth === 0 ? "●" : "└─▶"}</span>
          <span className="agent">
            {run.project}/{run.agent}
          </span>
          <StateBadge run={run} />
          <span className="muted small">{age(run.created_at)} ago</span>
          {run.cost_usd !== null && <span className="muted small">{cost(run.cost_usd)}</span>}
          {chain_cost && chain_cost.total > 0 && (
            <span className="chaincost" title="Total for this chain">
              Σ {cost(chain_cost.total)}
              {chain_cost.partial ? "+" : ""}
            </span>
          )}
          {/* Act where you noticed the problem, rather than going back
              to a list to do it. */}
          <span className="row-actions">
            {isActive(run.state) ? (
              <button
                className="tiny danger"
                onClick={(e) => {
                  e.stopPropagation();
                  void kill(run.id);
                }}
              >
                Stop
              </button>
            ) : (
              (run.state === "failed" || run.state === "timedout" || run.state === "killed") && (
                <button
                  className="tiny"
                  title="Re-run this stage against its parent's output"
                  onClick={(e) => {
                    e.stopPropagation();
                    void retry(run.id);
                  }}
                >
                  Retry
                </button>
              )
            )}
          </span>
        </li>
      ))}
    </ul>
  );
}
