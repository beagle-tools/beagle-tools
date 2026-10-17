import { useEffect, useState, type ReactElement } from "react";

import { age, cost, duration } from "../lib/format";
import { errorMessage, ipc } from "../lib/ipc";
import { treePrefixes } from "../lib/tree";
import type { ChainView, PipelineDetail, PipelineSummary, Run } from "../lib/types";
import { useStore } from "../state/store";
import { StepPane } from "./StepPane";

type Tab = "shape" | "runs";

// One job, the way agenttop shows it. Shape: every branch the job has
// taken, as a tree. Runs: every run, each behind its own partition, its
// steps beneath — and the step you click shows its log right beside the
// list. That last part is the fix: clicking a step used to only SELECT
// it, with the log behind a separate tab, so the click looked like it
// did nothing at all.
export function JobPage({
  job,
  onBack,
}: {
  job: PipelineSummary;
  onBack: () => void;
}): ReactElement {
  const forest = useStore((s) => s.forest);
  const selectedId = useStore((s) => s.selectedId);
  const select = useStore((s) => s.select);

  const [detail, setDetail] = useState<PipelineDetail | null>(null);
  const [tab, setTab] = useState<Tab>("runs");

  // Re-read whenever the poller says something changed (`forest` moves on
  // every change), so a run in flight grows its steps while you watch.
  useEffect(() => {
    let cancelled = false;
    ipc
      .pipelineDetail(job.project, job.name)
      .then((d) => {
        if (!cancelled) setDetail(d);
      })
      .catch((e: unknown) => {
        if (!cancelled) useStore.setState({ error: errorMessage(e) });
      });
    return () => {
      cancelled = true;
    };
  }, [job.project, job.name, forest]);

  const steps: Run[] = detail?.chains.flatMap((c) => c.steps.map((s) => s.run)) ?? [];
  const selected = steps.find((r) => r.id === selectedId) ?? null;

  // Open on something: the newest run's first step, so the log pane is
  // never an empty box asking you to go and find a step first.
  useEffect(() => {
    if (!detail || selected) return;
    const first = detail.chains[0]?.steps[0]?.run;
    if (first) void select(first.id);
  }, [detail, selected, select]);

  return (
    <div className="page job">
      <div className="panel">
        <div className="panel-head">
          <button className="back" onClick={onBack} title="All jobs">
            ‹
          </button>
          <h2>
            {job.project} / {job.name}
          </h2>
          <div className="seg">
            <button
              className={tab === "shape" ? "on" : ""}
              onClick={() => {
                setTab("shape");
              }}
            >
              Shape
            </button>
            <button
              className={tab === "runs" ? "on" : ""}
              onClick={() => {
                setTab("runs");
              }}
            >
              Runs{detail ? ` (${String(detail.chains.length)})` : ""}
            </button>
          </div>
          <div className="panel-actions">
            {job.active_runs > 0 && (
              <span className="pill running">
                running · {job.active_steps.join(", ") || "starting"}
              </span>
            )}
          </div>
        </div>

        <div className="panel-body">
          {!detail ? (
            <div className="empty muted">Loading…</div>
          ) : tab === "shape" ? (
            <Shape detail={detail} />
          ) : detail.chains.length === 0 ? (
            <div className="empty muted">
              {job.name} has not run yet. Start it with Run job, or `ag -j {job.name}`.
            </div>
          ) : (
            <div className="runsplit">
              <div className="runlist-col">
                {detail.chains.map((chain) => (
                  <ChainBlock
                    key={chain.root.id}
                    chain={chain}
                    selectedId={selectedId}
                    onPick={(id) => void select(id)}
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

// Every branch the job has ever taken. A step reachable two ways
// (linear-report, under implement and under investigate) shows under each:
// that is the shape, not duplication.
function Shape({ detail }: { detail: PipelineDetail }) {
  const prefixes = treePrefixes(detail.flow.map((n) => n.depth));
  return (
    <div className="shape">
      <p className="muted small">
        Every path this job has taken across all its runs. Open Runs to see what each run did.
      </p>
      <pre className="flowtree big">
        {detail.flow.map((node, i) => (
          <div
            key={`${String(i)}-${node.agent}`}
            className={detail.active_steps.includes(node.agent) ? "live" : undefined}
          >
            <span className="guide">{prefixes[i]}</span>
            <span className={i === 0 ? "root" : undefined}>{node.agent}</span>
          </div>
        ))}
      </pre>
    </div>
  );
}

// One run: a ruled partition with when, how it ended and the chain total,
// then its steps as a tree. The partition is what keeps two runs from
// reading as one long list.
function ChainBlock({
  chain,
  selectedId,
  onPick,
}: {
  chain: ChainView;
  selectedId: string | null;
  onPick: (id: string) => void;
}) {
  const prefixes = treePrefixes(chain.steps.map((s) => s.depth));
  const when = new Date(chain.root.created_at);
  return (
    <section className="chainblock">
      <header className={`partition ${chain.root.state}`}>
        <span className="when">
          {when.toLocaleDateString(undefined, { month: "short", day: "numeric" })}{" "}
          {when.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" })}
        </span>
        <span>{chain.root.state}</span>
        <span className="muted">{age(chain.root.created_at)} ago</span>
        {chain.cost.total > 0 && (
          <span className="chain-total">
            Σ {cost(chain.cost.total)}
            {chain.cost.partial ? "+" : ""}
          </span>
        )}
      </header>
      <ul className="steprows">
        {chain.steps.map(({ run }, i) => (
          <li
            key={run.id}
            className={`${run.state}${run.id === selectedId ? " selected" : ""}`}
            onClick={() => {
              onPick(run.id);
            }}
          >
            <span className="guide">{prefixes[i]}</span>
            <span className="stepname">{run.agent}</span>
            <span className={`badge ${run.state}`}>{run.state}</span>
            <span className="muted small num">{duration(run.started_at, run.finished_at)}</span>
            <span className="muted small num">
              {run.cost_usd !== null ? cost(run.cost_usd) : ""}
            </span>
          </li>
        ))}
      </ul>
    </section>
  );
}
