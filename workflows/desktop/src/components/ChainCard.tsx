import type { ReactElement } from "react";
import { age, cost } from "../lib/format";
import { edgePath, layout, type Chain } from "../lib/graph";
import { RunNode } from "./RunNode";

// One chain, separated from the next by a rule and whitespace — not
// wrapped in a card. The graph was already readable; what it lacked was
// a boundary. Giving each chain a bordered panel with a filled header,
// status pills and a stage count added far more chrome than the problem
// called for, and at eighteen chains the chrome WAS the page.
//
// So: a thin caption line, a hairline, the graph. Everything the header
// used to spell out is already in the nodes.
export function ChainCard({ rows }: { rows: Chain }): ReactElement {
  const root = rows[0];
  const graph = layout(rows);
  const pad = 14;

  const broken = rows.find(
    (r) => r.run.state === "failed" || r.run.state === "timedout" || r.run.state === "killed",
  );
  const running = rows.some((r) => r.run.state === "running" || r.run.state === "pending");
  const chainCost = root.chain_cost;

  return (
    <section className="chain">
      <div className="chain-rule">
        <span className="chain-name">{root.run.agent}</span>
        <span className="muted small">
          {root.run.project} · {age(root.run.created_at)} ago
        </span>
        {running && <span className="muted small accent">running</span>}
        {broken && (
          <span className="small err">
            {broken.run.agent} {broken.run.state}
          </span>
        )}
        {chainCost && chainCost.total > 0 && (
          <span className="chain-total" title={chainCost.partial ? "still running — a floor" : ""}>
            Σ {cost(chainCost.total)}
            {chainCost.partial ? "+" : ""}
          </span>
        )}
      </div>

      <svg width={graph.width + pad * 2} height={graph.height + pad * 2} role="img">
        <g transform={`translate(${String(pad)}, ${String(pad)})`}>
          {graph.edges.map((edge) => (
            <path
              key={`${edge.from.row.run.id}->${edge.to.row.run.id}`}
              d={edgePath(edge)}
              className={`edge ${edge.to.row.run.state}`}
            />
          ))}
          {graph.nodes.map((node) => (
            <RunNode key={node.row.run.id} node={node} />
          ))}
        </g>
      </svg>
    </section>
  );
}
