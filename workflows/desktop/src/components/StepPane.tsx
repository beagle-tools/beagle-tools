import { openUrl } from "@tauri-apps/plugin-opener";
import { useEffect, useState, type ReactElement } from "react";

import { cost, duration } from "../lib/format";
import { isActive, type Run } from "../lib/types";
import { useStore } from "../state/store";
import { LogView } from "./LogView";
import { MarkdownView } from "./MarkdownView";

// One step, beside the runs list: what it is in a line, the things it
// links to, and its log — open the moment you click the step.
//
// Takes the run as a prop rather than looking it up in the store's run
// list. That list is capped at 200 across every agent, and looking a
// step up there is why an older run's step used to show "Select a run.".
export function StepPane({ run }: { run: Run }): ReactElement {
  const links = useStore((s) => s.links);
  const kill = useStore((s) => s.kill);
  const retry = useStore((s) => s.retry);
  const [writeup, setWriteup] = useState<string | null>(null);

  // A new step means its own log, not the last step's write-up.
  useEffect(() => {
    setWriteup(null);
  }, [run.id]);

  const broken = run.state === "failed" || run.state === "timedout" || run.state === "killed";

  return (
    <div className="steppane">
      <header className="stephead">
        <div className="stephead-line">
          <strong>{run.agent}</strong>
          <span className={`badge ${run.state}`}>{run.state}</span>
          <span className="muted small">
            {duration(run.started_at, run.finished_at)}
            {run.cost_usd !== null ? ` · ${cost(run.cost_usd)}` : ""}
            {run.num_turns !== null ? ` · ${String(run.num_turns)} tool calls` : ""}
            {run.exit_code !== null && run.exit_code !== 0
              ? ` · exit ${String(run.exit_code)}`
              : ""}
          </span>
          <span className="stephead-actions">
            {isActive(run.state) && (
              <button className="tiny danger" onClick={() => void kill(run.id)}>
                Stop
              </button>
            )}
            {broken && (
              <button
                className="tiny"
                title="Re-run this step against its parent's output"
                onClick={() => void retry(run.id)}
              >
                Retry
              </button>
            )}
          </span>
        </div>
        <code className="muted small">{run.id}</code>

        {links.length > 0 && (
          <div className="steplinks">
            {writeup && (
              <button
                className="tiny"
                onClick={() => {
                  setWriteup(null);
                }}
              >
                ‹ log
              </button>
            )}
            {links.map((link) => (
              <button
                key={`${link.kind}:${link.target}`}
                className={`tiny${writeup === link.target ? " on" : ""}`}
                onClick={() => {
                  if (link.kind === "url") void openUrl(link.target);
                  else setWriteup(link.target);
                }}
              >
                {link.label}
              </button>
            ))}
          </div>
        )}
      </header>

      <div className="stepbody">{writeup ? <MarkdownView path={writeup} /> : <LogView />}</div>
    </div>
  );
}
