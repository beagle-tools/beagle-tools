import type { ReactElement } from "react";
import { chains } from "../lib/graph";
import type { ForestRow } from "../lib/types";
import { ChainCard } from "./ChainCard";

export function RunGraph({ rows }: { rows: ForestRow[] }): ReactElement {
  if (rows.length === 0) return <div className="empty muted">No runs to graph.</div>;

  return (
    <div className="chains">
      {chains(rows).map((chain) => (
        <ChainCard key={chain[0].run.id} rows={chain} />
      ))}
    </div>
  );
}
