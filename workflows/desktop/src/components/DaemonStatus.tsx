import type { ReactElement } from "react";

import { useStore } from "../state/store";

// Always visible, on every page: is agentd up, and is anything running.
// A stopped daemon leaves the store looking merely idle, so it wins.
export function TopStatus(): ReactElement {
  const daemon = useStore((s) => s.daemon);
  const pipelines = useStore((s) => s.pipelines);
  const anyRunning = pipelines.filter((p) => p.active_runs > 0);
  return (
    <span className={daemon ? "topstatus small daemon-down" : "topstatus muted small"}>
      {daemon
        ? "agentd stopped"
        : anyRunning.length === 0
          ? "idle"
          : `${String(anyRunning.length)} running · ${anyRunning
              .flatMap((p) => p.active_steps)
              .join(", ")}`}
    </span>
  );
}

// Not dismissable: it stays until agentd is running again, with the
// command that brings it back.
export function DaemonBar(): ReactElement | null {
  const daemon = useStore((s) => s.daemon);
  if (!daemon) return null;
  return (
    <div className="daemonbar">
      <strong>■ {daemon}</strong>
      <code>agentctl daemon restart</code>
    </div>
  );
}
