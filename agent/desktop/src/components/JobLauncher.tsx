import { useMemo, useState, type ReactElement } from "react";

import type { AgentSpec } from "../lib/types";
import { useStore } from "../state/store";

// Mirrors `ag -j <job> -p k=v`: the required parameters are enforced
// before anything is queued. The backend checks them too — this just
// means you find out while typing instead of after clicking.
export function JobLauncher({ onClose }: { onClose: () => void }): ReactElement {
  const jobs = useStore((s) => s.jobs);
  const trigger = useStore((s) => s.trigger);
  const select = useStore((s) => s.select);

  const [picked, setPicked] = useState<AgentSpec | null>(null);
  const [values, setValues] = useState<Record<string, string>>({});
  const [model, setModel] = useState("");
  const [busy, setBusy] = useState(false);

  const missing = useMemo(
    () => (picked?.params ?? []).filter((p) => p.required && !values[p.name]?.trim()),
    [picked, values],
  );

  async function run() {
    if (!picked || missing.length > 0) return;
    setBusy(true);
    const filled = Object.fromEntries(Object.entries(values).filter(([, v]) => v.trim() !== ""));
    const id = await trigger(picked.project, picked.name, filled, model.trim() || null);
    setBusy(false);
    if (id) {
      await select(id);
      onClose();
    }
  }

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div
        className="modal"
        onClick={(e) => {
          e.stopPropagation();
        }}
      >
        <h2>Run a job</h2>

        <div className="joblist">
          {jobs.map((job) => (
            <button
              key={`${job.project}/${job.name}`}
              className={picked === job ? "job selected" : "job"}
              onClick={() => {
                setPicked(job);
                setValues({});
              }}
            >
              <span className="agent">{job.name}</span>
              <span className="muted small">{job.project}</span>
              <span className="muted small">{job.runner}</span>
              {job.params.some((p) => p.required) && <span className="req">needs params</span>}
            </button>
          ))}
        </div>

        {picked && (
          <div className="jobform">
            {picked.params.length === 0 && <p className="muted small">No parameters.</p>}
            {picked.params.map((p) => (
              <label key={p.name}>
                <span>
                  {p.name}
                  {p.required && <b className="req"> *</b>}
                </span>
                <input
                  value={values[p.name] ?? ""}
                  placeholder={p.description ?? ""}
                  onChange={(e) => {
                    setValues({ ...values, [p.name]: e.target.value });
                  }}
                />
              </label>
            ))}
            <label>
              <span>model</span>
              <input
                value={model}
                placeholder={picked.model ?? "manifest default"}
                onChange={(e) => {
                  setModel(e.target.value);
                }}
              />
            </label>
          </div>
        )}

        <footer>
          {picked && missing.length > 0 && (
            <span className="muted small">needs {missing.map((p) => p.name).join(", ")}</span>
          )}
          <button onClick={onClose}>Cancel</button>
          <button
            className="primary"
            disabled={!picked || missing.length > 0 || busy}
            onClick={() => void run()}
          >
            {busy ? "Queueing…" : "Run"}
          </button>
        </footer>
      </div>
    </div>
  );
}
