import type { ReactElement } from "react";
import { age, cost, duration } from "../lib/format";
import { isActive, type Run } from "../lib/types";
import { useStore } from "../state/store";

// Rows, not a table. Four columns in a 330px sidebar left nothing for
// the agent name, which is the one thing you actually scan for —
// `linear-implement` was truncating to make room for a cost column that
// is blank on half the rows.
export function RunList(): ReactElement {
  const runs = useStore((s) => s.runs);

  if (runs.length === 0) {
    return (
      <div className="empty">
        <p>No runs yet.</p>
        <p className="muted small">Start one with Run job, or `ag -j &lt;job&gt;`.</p>
      </div>
    );
  }

  const active = runs.filter((r) => isActive(r.state));
  const finished = runs.filter((r) => !isActive(r.state));

  return (
    <div className="runlist">
      {active.length > 0 && <Group label="Active" runs={active} />}
      {finished.length > 0 && <Group label="Recent" runs={finished} />}
    </div>
  );
}

// The split is explicit rather than implied by sort order: "is anything
// running right now" is the question you open this app with.
function Group({ label, runs }: { label: string; runs: Run[] }) {
  return (
    <>
      <div className="group-head">
        <span>{label}</span>
        <span className="count">{runs.length}</span>
      </div>
      <ul>
        {runs.map((run) => (
          <RunRow key={run.id} run={run} />
        ))}
      </ul>
    </>
  );
}

function RunRow({ run }: { run: Run }) {
  const selectedId = useStore((s) => s.selectedId);
  const select = useStore((s) => s.select);
  const failed = run.state === "failed" || run.state === "killed";

  return (
    <li
      className={`${run.state}${run.id === selectedId ? " selected" : ""}`}
      onClick={() => void select(run.id)}
    >
      <span className="dot" aria-hidden />
      <div className="rl-body">
        <div className="rl-line">
          <span className="rl-agent" title={run.id}>
            {run.agent}
          </span>
          <span className="rl-right num">{duration(run.started_at, run.finished_at)}</span>
        </div>
        <div className="rl-line">
          <span className="muted small rl-sub">
            {run.project} · {age(run.created_at)} ago
            {run.trigger === "cron" && " · cron"}
          </span>
          <span className="rl-right small">
            {failed && run.exit_code !== null ? (
              <span className="exit">exit {run.exit_code}</span>
            ) : (
              <span className="muted num">{cost(run.cost_usd)}</span>
            )}
          </span>
        </div>
      </div>
    </li>
  );
}

// Still used by the detail header, the flows list and the topology pane.
export function StateBadge({ run }: { run: Run }): ReactElement {
  const suffix =
    run.state === "failed" && run.exit_code !== null ? ` ${String(run.exit_code)}` : "";
  return (
    <span className={`badge ${run.state}${isActive(run.state) ? " pulse" : ""}`}>
      {run.state}
      {suffix}
    </span>
  );
}
