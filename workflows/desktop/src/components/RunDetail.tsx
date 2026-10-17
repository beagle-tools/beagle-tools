import type { ReactElement } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";

import { cost, duration, params as parseParams } from "../lib/format";
import { isActive } from "../lib/types";
import { useStore } from "../state/store";
import { StateBadge } from "./RunList";

export function RunDetail({
  onOpenMarkdown,
}: {
  onOpenMarkdown: (path: string) => void;
}): ReactElement {
  const runs = useStore((s) => s.runs);
  const selectedId = useStore((s) => s.selectedId);
  const links = useStore((s) => s.links);
  const kill = useStore((s) => s.kill);

  const run = runs.find((r) => r.id === selectedId);
  if (!run) return <div className="empty muted">Select a run.</div>;

  const runParams = parseParams(run.params);

  return (
    <div className="detail">
      <header>
        <div>
          <h2>
            {run.project}/{run.agent}
          </h2>
          <code className="muted small">{run.id}</code>
        </div>
        <div className="detail-actions">
          <StateBadge run={run} />
          {isActive(run.state) && (
            <button className="danger" onClick={() => void kill(run.id)}>
              Kill
            </button>
          )}
        </div>
      </header>

      <dl className="meta">
        <Field label="Trigger" value={run.trigger} />
        <Field label="Model" value={run.model ?? "manifest default"} />
        <Field label="Duration" value={duration(run.started_at, run.finished_at)} />
        <Field label="Cost" value={cost(run.cost_usd)} />
        <Field label="Tool calls" value={run.num_turns === null ? "—" : String(run.num_turns)} />
        <Field label="tmux" value={run.tmux_session ?? "—"} />
      </dl>

      {Object.keys(runParams).length > 0 && (
        <>
          <h3>Parameters</h3>
          <dl className="meta">
            {Object.entries(runParams).map(([k, v]) => (
              <Field key={k} label={k} value={v} />
            ))}
          </dl>
        </>
      )}

      <h3>Output</h3>
      <LogTail />

      <h3>Links</h3>
      {links.length === 0 ? (
        <p className="muted small">
          Nothing linked. Write-ups and data dirs are swept by retention, so an older run keeps only
          what lives on its row.
        </p>
      ) : (
        <ul className="links">
          {links.map((link) => (
            <li key={`${link.kind}:${link.target}`}>
              <button
                onClick={() => {
                  if (link.kind === "url") void openUrl(link.target);
                  else onOpenMarkdown(link.target);
                }}
              >
                {link.label}
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

const TAIL_LINES = 18;

function LogTail() {
  const log = useStore((s) => s.log);
  if (!log) return <p className="muted small">Loading…</p>;
  if (log.missing_reason) return <p className="muted small">{log.missing_reason}</p>;
  if (log.entries.length === 0) return <p className="muted small">Nothing written yet.</p>;

  // The same readable entries the Log tab shows, the newest few — not the
  // raw file, which for a model run is a wall of stream-json.
  const tail = log.entries.slice(-TAIL_LINES);
  return (
    <>
      <pre className="logtail">
        {tail
          .map((e) => `${e.tag.padEnd(11)} ${e.source ? `${e.source} ` : ""}${e.body}`)
          .join("\n")}
      </pre>
      <p className="muted small">
        last {tail.length} of {log.entries.length} — the Log tab has all of it, coloured and
        searchable
      </p>
    </>
  );
}

function Field({ label, value }: { label: string; value: string }) {
  return (
    <>
      <dt>{label}</dt>
      <dd>{value}</dd>
    </>
  );
}
