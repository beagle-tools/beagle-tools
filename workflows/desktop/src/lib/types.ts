// Mirrors the Rust serde output. Fields are snake_case because agent-core
// does not rename them, and the enums are lowercased variant names —
// note "timedout", which has no separator.
//
// Keep this in sync with crates/agent-core/src/types.rs by hand. There is
// no codegen step, so a field added there and not here is simply invisible
// to the UI rather than a build error.

export type RunState = "pending" | "running" | "succeeded" | "failed" | "timedout" | "killed";

export type Trigger = "cron" | "manual";

export interface Run {
  id: string;
  project: string;
  agent: string;
  state: RunState;
  trigger: Trigger;
  parent_run_id: string | null;
  params: string | null;
  tmux_session: string | null;
  log_path: string | null;
  workdir: string | null;
  model: string | null;
  created_at: string;
  started_at: string | null;
  finished_at: string | null;
  exit_code: number | null;
  cost_usd: number | null;
  num_turns: number | null;
  usage_json: string | null;
}

export interface ParamSpec {
  name: string;
  required: boolean;
  default: string | null;
  description: string | null;
}

export interface AgentSpec {
  project: string;
  name: string;
  /** Path to the manifest that declared it (PathBuf -> string). */
  source: string;
  runner: string;
  model: string | null;
  schedule: string | null;
  params: ParamSpec[];
  enabled: boolean;
  timeout_minutes: number;
}

/** One readable line of a run — see agent_core::transcript. */
export interface TranscriptEntry {
  ts: string;
  /** What kind of line this is; drives its colour. A model run uses
   *  assistant / thinking / tool_use / tool_result / result / denied;
   *  a script run uses info / step / ok / warn / error / out. Both use
   *  state, note and clipped. */
  tag: string;
  body: string;
  /** For a script line, who said it (`gather` in `gather: wrote …`). */
  source: string | null;
}

export interface Transcript {
  entries: TranscriptEntry[];
  /** A model run (a conversation) rather than a script (a log). */
  agent: boolean;
  missing_reason: string | null;
}

export interface Link {
  label: string;
  kind: "markdown" | "url" | "file";
  target: string;
}

export const ACTIVE_STATES: RunState[] = ["pending", "running"];

export function isActive(state: RunState): boolean {
  return ACTIVE_STATES.includes(state);
}

export interface SubtreeCost {
  total: number;
  partial: boolean;
}

export interface ForestRow {
  run: Run;
  depth: number;
  /** Only on a root: what the whole chain cost. */
  chain_cost: SubtreeCost | null;
  children: string[];
}

export interface AgentNode {
  spec: AgentSpec;
  latest: Run | null;
}

/** One node of a job's flow: an agent at a depth, entry point first. */
export interface FlowNode {
  agent: string;
  depth: number;
}

/** A job you start, as opposed to a step something else triggers. */
export interface PipelineSummary {
  project: string;
  name: string;
  runner: string;
  schedule: string | null;
  enabled: boolean;
  /** The job's shape — every branch its chains have taken. */
  flow: FlowNode[];
  steps: string[];
  active_steps: string[];
  active_runs: number;
  last: Run | null;
}

export interface StepRow {
  run: Run;
  depth: number;
}

/** One run of a job, with every step it ran. */
export interface ChainView {
  root: Run;
  cost: SubtreeCost;
  steps: StepRow[];
}

export interface PipelineDetail {
  project: string;
  name: string;
  flow: FlowNode[];
  active_steps: string[];
  /** Newest first. */
  chains: ChainView[];
}

/** One published Agentdesk version (`commands::updates::AppRelease`). */
export interface AppRelease {
  version: string;
  tag: string;
  published_at: string | null;
  /** False when the release has no updater manifest to install from. */
  installable: boolean;
}
