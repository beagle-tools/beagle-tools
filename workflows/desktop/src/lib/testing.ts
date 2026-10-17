// Fixture builders shared by the unit tests. Not imported by the app.

import type { ForestRow, Run } from "./types";

/** A finished-looking run; override only the fields a test cares about. */
export function makeRun(overrides: Partial<Run> = {}): Run {
  return {
    id: "run-1",
    project: "demo",
    agent: "writer",
    state: "succeeded",
    trigger: "manual",
    parent_run_id: null,
    params: null,
    tmux_session: null,
    log_path: null,
    workdir: null,
    model: null,
    created_at: "2026-01-01T00:00:00Z",
    started_at: null,
    finished_at: null,
    exit_code: null,
    cost_usd: null,
    num_turns: null,
    usage_json: null,
    ...overrides,
  };
}

/** A forest row for run `id` at `depth` with the given child run ids. */
export function makeRow(id: string, depth: number, children: string[] = []): ForestRow {
  return { run: makeRun({ id }), depth, chain_cost: null, children };
}
