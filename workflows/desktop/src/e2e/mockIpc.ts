// e2e IPC mock: answers every Tauri command the UI sends from fixture data,
// so Playwright can drive the real frontend in a plain browser. main.tsx
// imports this only when vite runs with `--mode e2e`; production builds
// drop it (and the fixtures) entirely.
//
// It is a tiny in-memory agentd: `trigger_job` enforces required params the
// way the Rust command does and queues a pending run that the jobs, the
// activity view and the job page all pick up.
//
// Knobs (query string on the page URL):
//   ?update=X.Y.Z   the updater reports X.Y.Z as available (default: none)
//
// Every trigger_job call is recorded on `window.__agentdeskE2E.triggers`
// so a spec can check exactly what the UI sent.

import type { InvokeArgs } from "@tauri-apps/api/core";
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";

import { makeRun } from "../lib/testing";
import type { ForestRow, PipelineDetail, PipelineSummary, Run } from "../lib/types";
import {
  AGENTS,
  APP_VERSION,
  FOREST,
  JOBS,
  LINKS,
  MANIFEST_YAML,
  PIPELINES,
  RELEASES,
  RUNS,
  TRANSCRIPTS,
  TRIAGE_DETAIL,
} from "./fixtures";

type Args = Record<string, unknown>;

export interface TriggerCall {
  project: string;
  agent: string;
  params: Record<string, string>;
  model: string | null;
}

declare global {
  interface Window {
    __agentdeskE2E?: { triggers: TriggerCall[] };
  }
}

const runs: Run[] = [...RUNS];
const forest: ForestRow[] = [...FOREST];
const pipelines: PipelineSummary[] = PIPELINES.map((p) => ({ ...p }));
const triggers: TriggerCall[] = [];

function asArgs(payload: InvokeArgs | undefined): Args {
  return payload !== undefined && !Array.isArray(payload) && !ArrayBuffer.isView(payload)
    ? (payload as Args)
    : {};
}

const str = (a: Args, key: string): string => (typeof a[key] === "string" ? a[key] : "");

function triggerJob(a: Args): string {
  const call: TriggerCall = {
    project: str(a, "project"),
    agent: str(a, "agent"),
    params: (a.params ?? {}) as Record<string, string>,
    model: typeof a.model === "string" ? a.model : null,
  };
  triggers.push(call);
  const spec = JOBS.find((j) => j.project === call.project && j.name === call.agent);
  if (!spec) throw new Error(`agent ${call.project}/${call.agent} is not in the store`);
  const missing = spec.params.filter((p) => p.required && !(p.name in call.params));
  if (missing.length > 0) {
    throw new Error(`${call.agent} needs: ${missing.map((p) => p.name).join(", ")}`);
  }
  const run = makeRun({
    id: `run-${call.agent}-e2e-${String(triggers.length)}`,
    project: call.project,
    agent: call.agent,
    state: "pending",
    params: JSON.stringify(call.params),
    model: call.model,
    created_at: new Date().toISOString(),
  });
  runs.unshift(run);
  forest.unshift({ run, depth: 0, chain_cost: null, children: [] });
  const job = pipelines.find((p) => p.project === call.project && p.name === call.agent);
  if (job) {
    job.last = run;
    job.active_runs += 1;
    job.active_steps = [call.agent];
  }
  return run.id;
}

function pipelineDetail(a: Args): PipelineDetail {
  const name = str(a, "name");
  if (name === TRIAGE_DETAIL.name) return TRIAGE_DETAIL;
  const own = runs.filter((r) => r.agent === name && r.parent_run_id === null);
  return {
    project: str(a, "project"),
    name,
    flow: [{ agent: name, depth: 0 }],
    active_steps: own.some((r) => r.state === "pending") ? [name] : [],
    chains: own.map((root) => ({
      root,
      cost: { total: 0, partial: false },
      steps: [{ run: root, depth: 0 }],
    })),
  };
}

/** Tauri plugin commands (`plugin:<name>|<cmd>`) the UI relies on. */
function plugin(cmd: string): unknown {
  switch (cmd) {
    case "plugin:app|version":
      return APP_VERSION;
    case "plugin:updater|check": {
      const version = new URLSearchParams(window.location.search).get("update");
      if (version === null) return null;
      return {
        rid: 1,
        currentVersion: APP_VERSION,
        version,
        date: "2026-10-16T00:00:00Z",
        body: "",
        rawJson: {},
      };
    }
    default:
      // window/opener/process calls: accept silently.
      return null;
  }
}

function handle(cmd: string, payload?: InvokeArgs): unknown {
  if (cmd.startsWith("plugin:")) return plugin(cmd);
  const a = asArgs(payload);
  const id = str(a, "id");
  switch (cmd) {
    case "list_runs":
      return runs;
    case "get_run":
      return runs.find((r) => r.id === id) ?? null;
    case "run_chain":
      return runs.filter((r) => r.id === id || r.parent_run_id === id);
    case "run_forest":
      return forest;
    case "list_pipelines":
      return pipelines;
    case "pipeline_detail":
      return pipelineDetail(a);
    case "list_jobs":
      return JOBS;
    case "trigger_job":
      return triggerJob(a);
    case "read_transcript":
      return (
        TRANSCRIPTS[id] ?? { entries: [], agent: false, missing_reason: "No log yet — queued." }
      );
    case "run_links":
      return LINKS[id] ?? [];
    case "agent_graph":
      return AGENTS;
    case "read_manifest":
      return MANIFEST_YAML;
    case "read_markdown":
      return "# Report\n\nAll good.";
    case "list_app_releases":
      return RELEASES;
    case "retry_run":
      return "run-retry-e2e";
    default:
      // kill_run, install_app_version: writes that succeed with no payload.
      return null;
  }
}

/** Install the mock before the app renders. */
export function installIpcMocks(): void {
  window.__agentdeskE2E = { triggers };
  mockWindows("main");
  mockIPC(handle, { shouldMockEvents: true });
}
