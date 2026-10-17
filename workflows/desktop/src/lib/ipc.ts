// Typed wrappers around Tauri invoke — the only place command names and
// argument shapes appear. A command renamed in Rust breaks here, in one
// file, rather than in whichever component happened to call it.

import { invoke } from "@tauri-apps/api/core";

import type {
  AgentNode,
  AgentSpec,
  ForestRow,
  Link,
  PipelineDetail,
  PipelineSummary,
  Run,
  Transcript,
} from "./types";

export const ipc = {
  listRuns: (limit?: number): Promise<Run[]> => invoke<Run[]>("list_runs", { limit }),
  getRun: (id: string): Promise<Run | null> => invoke<Run | null>("get_run", { id }),
  runChain: (id: string): Promise<Run[]> => invoke<Run[]>("run_chain", { id }),
  killRun: (id: string): Promise<null> => invoke<null>("kill_run", { id }),
  retryRun: (id: string): Promise<string> => invoke<string>("retry_run", { id }),

  readTranscript: (id: string): Promise<Transcript> =>
    invoke<Transcript>("read_transcript", { id }),

  listJobs: (): Promise<AgentSpec[]> => invoke<AgentSpec[]>("list_jobs"),
  listPipelines: (): Promise<PipelineSummary[]> => invoke<PipelineSummary[]>("list_pipelines"),
  pipelineDetail: (project: string, name: string): Promise<PipelineDetail> =>
    invoke<PipelineDetail>("pipeline_detail", { project, name }),
  triggerJob: (
    project: string,
    agent: string,
    params: Record<string, string>,
    model: string | null,
  ): Promise<string> => invoke<string>("trigger_job", { project, agent, params, model }),

  runForest: (focus: string | null): Promise<ForestRow[]> =>
    invoke<ForestRow[]>("run_forest", { focus }),
  agentGraph: (): Promise<AgentNode[]> => invoke<AgentNode[]>("agent_graph"),
  readManifest: (project: string, agent: string): Promise<string> =>
    invoke<string>("read_manifest", { project, agent }),

  runLinks: (id: string): Promise<Link[]> => invoke<Link[]>("run_links", { id }),
  readMarkdown: (path: string): Promise<string> => invoke<string>("read_markdown", { path }),
};

// Tauri returns our AppError struct, so an IPC failure arrives as
// { message }. Anything else is a bug in the bridge; show it rather than
// swallowing it into a generic string.
export function errorMessage(e: unknown): string {
  if (typeof e === "object" && e !== null && "message" in e) {
    return String(e.message);
  }
  return String(e);
}
