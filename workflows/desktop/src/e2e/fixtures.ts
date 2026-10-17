// Fixture data for the e2e IPC mock: one machine's worth of agentd state.
// Project `demo` declares two jobs:
//   triage  (claude, cron) → fix → report   — has run twice; the latest
//                                              chain's report step failed
//   hello   (script, manual, needs `name`)   — has never run
// Shapes mirror the Rust serde output (see src/lib/types.ts).

import { makeRun } from "../lib/testing";
import type {
  AgentNode,
  AgentSpec,
  AppRelease,
  ForestRow,
  Link,
  PipelineDetail,
  PipelineSummary,
  Run,
  Transcript,
} from "../lib/types";

export const APP_VERSION = "0.2.0";

const AT = "2026-10-16T09:00:00Z";

export const RUNS: Run[] = [
  makeRun({
    id: "run-triage-2",
    agent: "triage",
    trigger: "cron",
    model: "sonnet",
    created_at: "2026-10-16T09:00:00Z",
    started_at: "2026-10-16T09:00:01Z",
    finished_at: "2026-10-16T09:03:30Z",
    exit_code: 0,
    cost_usd: 0.42,
    num_turns: 12,
    tmux_session: "agentd-run-triage-2",
  }),
  makeRun({
    id: "run-fix-2",
    agent: "fix",
    parent_run_id: "run-triage-2",
    created_at: "2026-10-16T09:03:31Z",
    started_at: "2026-10-16T09:03:32Z",
    finished_at: "2026-10-16T09:10:00Z",
    exit_code: 0,
    cost_usd: 1.1,
  }),
  makeRun({
    id: "run-report-2",
    agent: "report",
    state: "failed",
    parent_run_id: "run-fix-2",
    created_at: "2026-10-16T09:10:01Z",
    started_at: "2026-10-16T09:10:02Z",
    finished_at: "2026-10-16T09:10:20Z",
    exit_code: 1,
  }),
  makeRun({
    id: "run-triage-1",
    agent: "triage",
    trigger: "cron",
    created_at: "2026-10-15T09:00:00Z",
    started_at: "2026-10-15T09:00:01Z",
    finished_at: "2026-10-15T09:02:00Z",
    exit_code: 0,
    cost_usd: 0.3,
  }),
];

const byId = (id: string): Run => {
  const run = RUNS.find((r) => r.id === id);
  if (!run) throw new Error(`fixture run ${id} missing`);
  return run;
};

export const FOREST: ForestRow[] = [
  {
    run: byId("run-triage-2"),
    depth: 0,
    chain_cost: { total: 1.52, partial: false },
    children: ["run-fix-2"],
  },
  { run: byId("run-fix-2"), depth: 1, chain_cost: null, children: ["run-report-2"] },
  { run: byId("run-report-2"), depth: 2, chain_cost: null, children: [] },
  {
    run: byId("run-triage-1"),
    depth: 0,
    chain_cost: { total: 0.3, partial: false },
    children: [],
  },
];

function spec(overrides: Partial<AgentSpec> & Pick<AgentSpec, "name">): AgentSpec {
  return {
    project: "demo",
    source: `/Users/e2e/.config/agentd/manifests/demo/${overrides.name}.yaml`,
    runner: "claude",
    model: null,
    schedule: null,
    params: [],
    enabled: true,
    timeout_minutes: 30,
    ...overrides,
  };
}

export const JOBS: AgentSpec[] = [
  spec({
    name: "hello",
    runner: "script",
    params: [
      { name: "name", required: true, default: null, description: "who to greet" },
      { name: "greeting", required: false, default: "hi", description: "opening word" },
    ],
  }),
  spec({ name: "triage", model: "sonnet", schedule: "0 9 * * 1-5" }),
];

export const AGENTS: AgentNode[] = [
  { spec: JOBS[0] ?? spec({ name: "hello" }), latest: null },
  { spec: JOBS[1] ?? spec({ name: "triage" }), latest: byId("run-triage-2") },
  { spec: spec({ name: "fix" }), latest: byId("run-fix-2") },
  { spec: spec({ name: "report", runner: "script" }), latest: byId("run-report-2") },
];

const TRIAGE_FLOW = [
  { agent: "triage", depth: 0 },
  { agent: "fix", depth: 1 },
  { agent: "report", depth: 2 },
];

export const PIPELINES: PipelineSummary[] = [
  {
    project: "demo",
    name: "hello",
    runner: "script",
    schedule: null,
    enabled: true,
    flow: [{ agent: "hello", depth: 0 }],
    steps: ["hello"],
    active_steps: [],
    active_runs: 0,
    last: null,
  },
  {
    project: "demo",
    name: "triage",
    runner: "claude",
    schedule: "0 9 * * 1-5",
    enabled: true,
    flow: TRIAGE_FLOW,
    steps: ["triage", "fix", "report"],
    active_steps: [],
    active_runs: 0,
    last: byId("run-triage-2"),
  },
];

export const TRIAGE_DETAIL: PipelineDetail = {
  project: "demo",
  name: "triage",
  flow: TRIAGE_FLOW,
  active_steps: [],
  chains: [
    {
      root: byId("run-triage-2"),
      cost: { total: 1.52, partial: false },
      steps: [
        { run: byId("run-triage-2"), depth: 0 },
        { run: byId("run-fix-2"), depth: 1 },
        { run: byId("run-report-2"), depth: 2 },
      ],
    },
    {
      root: byId("run-triage-1"),
      cost: { total: 0.3, partial: false },
      steps: [{ run: byId("run-triage-1"), depth: 0 }],
    },
  ],
};

const entry = (tag: string, body: string, source: string | null = null) => ({
  ts: AT,
  tag,
  body,
  source,
});

export const TRANSCRIPTS: Record<string, Transcript> = {
  "run-triage-2": {
    agent: true,
    missing_reason: null,
    entries: [
      entry("assistant", "Looking at the open issues labelled bug."),
      entry("tool_use", "gh issue list --label bug --limit 5"),
      entry("tool_result", "#412 Webhook retries never back off"),
      entry("result", "Picked #412 for the fix stage."),
    ],
  },
  "run-fix-2": {
    agent: true,
    missing_reason: null,
    entries: [entry("assistant", "Added exponential backoff with jitter to the dispatcher.")],
  },
  "run-report-2": {
    agent: false,
    missing_reason: null,
    entries: [
      entry("step", "posting summary", "report"),
      entry("error", "slack webhook returned 403", "report"),
    ],
  },
};

export const LINKS: Record<string, Link[]> = {
  "run-fix-2": [
    { label: "PR #418", kind: "url", target: "https://github.com/acme/widgets/pull/418" },
  ],
};

export const MANIFEST_YAML = "kind: Agent\nname: triage\nrunner: claude\nschedule: 0 9 * * 1-5\n";

export const RELEASES: AppRelease[] = [
  {
    version: "0.3.0",
    tag: "beagle-workflows-v0.3.0",
    published_at: "2026-10-15T12:00:00Z",
    installable: true,
  },
  {
    version: "0.2.0",
    tag: "beagle-workflows-v0.2.0",
    published_at: "2026-10-01T12:00:00Z",
    installable: true,
  },
  {
    version: "0.1.0",
    tag: "beagle-workflows-v0.1.0",
    published_at: "2026-09-20T12:00:00Z",
    installable: true,
  },
];
