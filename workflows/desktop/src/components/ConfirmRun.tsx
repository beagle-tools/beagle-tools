import { useEffect, useMemo, useRef, useState, type ReactElement } from "react";
import { createPortal } from "react-dom";

import type { Run } from "../lib/types";
import { useStore } from "../state/store";

// The confirmation in front of anything that starts work — agenttop's `R`
// pane, as a dialog. Two kinds:
//
//   start  — run a job from its entry point. Asks for any required
//            parameter the manifest gives no default.
//   resume — run a failed step again in its chain (agent_core::resume):
//            same parent, params and model, so it picks up the handover.
//
// Enter confirms, Esc cancels, as in the TUI. Portalled to <body>, so it
// works from inside the run graph's <svg> as well as from a list.
export type RunRequest =
  { kind: "start"; project: string; name: string } | { kind: "resume"; run: Run };

export function ConfirmRun({
  request,
  onClose,
  onDone,
}: {
  request: RunRequest;
  onClose: () => void;
  onDone?: (newId: string) => void;
}): ReactElement {
  const jobs = useStore((s) => s.jobs);
  const trigger = useStore((s) => s.trigger);
  const retry = useStore((s) => s.retry);
  const [values, setValues] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState(false);
  const confirmBtn = useRef<HTMLButtonElement>(null);

  const needed = useMemo(() => {
    if (request.kind !== "start") return [];
    const spec = jobs.find((j) => j.project === request.project && j.name === request.name);
    return (spec?.params ?? []).filter((p) => p.required && p.default === null);
  }, [jobs, request]);
  const missing = needed.filter((p) => !values[p.name]?.trim());

  useEffect(() => {
    if (needed.length === 0) confirmBtn.current?.focus();
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
    };
  }, [needed.length, onClose]);

  async function confirm() {
    if (busy || missing.length > 0) return;
    setBusy(true);
    const id =
      request.kind === "start"
        ? await trigger(
            request.project,
            request.name,
            Object.fromEntries(Object.entries(values).filter(([, v]) => v.trim() !== "")),
            null,
          )
        : await retry(request.run.id);
    setBusy(false);
    if (id) onDone?.(id);
    onClose();
  }

  const title = request.kind === "start" ? "Run job" : "Restart from this step";

  return createPortal(
    <div className="modal-backdrop" onClick={onClose}>
      <div
        className="modal confirm"
        role="dialog"
        aria-label={title}
        onClick={(e) => {
          e.stopPropagation();
        }}
      >
        <h2>{title}</h2>
        {request.kind === "start" ? (
          <p>
            Run{" "}
            <strong>
              {request.project} / {request.name}
            </strong>{" "}
            now? Its whole chain runs, as the schedule would run it.
          </p>
        ) : (
          <p>
            Restart the chain at <strong>{request.run.agent}</strong>? It {request.run.state}; a new
            run of that step reads the same handover, with the same params and model, and the rest
            of the chain carries on from there.
          </p>
        )}
        {needed.map((p) => (
          <label key={p.name} className="field">
            <span>
              {p.name}
              {p.description && <span className="muted small"> — {p.description}</span>}
            </span>
            <input
              value={values[p.name] ?? ""}
              onChange={(e) => {
                setValues((v) => ({ ...v, [p.name]: e.target.value }));
              }}
              onKeyDown={(e) => {
                if (e.key === "Enter") void confirm();
              }}
            />
          </label>
        ))}
        <footer>
          <button onClick={onClose}>Cancel</button>
          <button
            ref={confirmBtn}
            className="primary"
            disabled={busy || missing.length > 0}
            title={missing.length > 0 ? `needs ${missing.map((p) => p.name).join(", ")}` : ""}
            onClick={() => void confirm()}
          >
            {request.kind === "start" ? "Run" : "Restart"}
          </button>
        </footer>
      </div>
    </div>,
    document.body,
  );
}

// The `⋯` on a job: a small menu whose one item, for now, is Run now.
export function JobMenu({ project, name }: { project: string; name: string }): ReactElement {
  const [open, setOpen] = useState(false);
  const [asking, setAsking] = useState(false);

  useEffect(() => {
    if (!open) return;
    const close = () => {
      setOpen(false);
    };
    window.addEventListener("click", close);
    return () => {
      window.removeEventListener("click", close);
    };
  }, [open]);

  return (
    <span
      className="jobmenu"
      onClick={(e) => {
        e.stopPropagation();
      }}
    >
      <button
        className="tiny ghost"
        aria-label={`Actions for ${name}`}
        aria-expanded={open}
        onClick={() => {
          setOpen((o) => !o);
        }}
      >
        ⋯
      </button>
      {open && (
        <div className="menu" role="menu">
          <button
            role="menuitem"
            onClick={() => {
              setOpen(false);
              setAsking(true);
            }}
          >
            Run now…
          </button>
        </div>
      )}
      {asking && (
        <ConfirmRun
          request={{ kind: "start", project, name }}
          onClose={() => {
            setAsking(false);
          }}
        />
      )}
    </span>
  );
}
