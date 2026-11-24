// Every agent run across all repos/PRs — powers the agents dashboard
// and its header badge. "Viewed" is persisted: once the user has jumped
// to a finished/errored run it counts as processed and leaves the
// needs-attention list (working runs never count as processed).

import { useMemo } from "react";
import { create } from "zustand";

import type { AgentRun } from "../lib/types";

const VIEWED_KEY = "tandem-viewed-runs";
const VIEWED_CAP = 500;

function loadViewed(): Set<string> {
  try {
    const raw = localStorage.getItem(VIEWED_KEY);
    const parsed: unknown = raw ? JSON.parse(raw) : [];
    return new Set(Array.isArray(parsed) ? parsed.filter((v) => typeof v === "string") : []);
  } catch {
    return new Set();
  }
}

function saveViewed(ids: Set<string>) {
  localStorage.setItem(VIEWED_KEY, JSON.stringify([...ids].slice(-VIEWED_CAP)));
}

export const isWorking = (r: AgentRun): boolean =>
  r.status === "starting" || r.status === "running";
export const isError = (r: AgentRun): boolean => r.status === "failed" || r.status === "timed_out";
export const isTerminal = (r: AgentRun): boolean => !isWorking(r);

interface RunBoardStore {
  /** run_id → latest known state of that run. */
  runs: Record<string, AgentRun>;
  viewed: Set<string>;
  ingest: (run: AgentRun) => void;
  ingestMany: (runs: AgentRun[]) => void;
  markViewed: (runId: string) => void;
  markAllViewed: () => void;
  remove: (runId: string) => void;
}

export const useRunBoard = create<RunBoardStore>((set, get) => ({
  runs: {},
  viewed: loadViewed(),

  ingest: (run) => {
    set((s) => ({ runs: { ...s.runs, [run.run_id]: run } }));
  },

  ingestMany: (runs) => {
    set((s) => {
      const next = { ...s.runs };
      for (const run of runs) next[run.run_id] = run;
      return { runs: next };
    });
  },

  markViewed: (runId) => {
    const viewed = new Set(get().viewed);
    viewed.add(runId);
    saveViewed(viewed);
    set({ viewed });
  },

  remove: (runId) => {
    set((s) => ({
      runs: Object.fromEntries(Object.entries(s.runs).filter(([id]) => id !== runId)),
    }));
  },

  markAllViewed: () => {
    const { runs } = get();
    const viewed = new Set(get().viewed);
    for (const run of Object.values(runs)) if (isTerminal(run)) viewed.add(run.run_id);
    saveViewed(viewed);
    set({ viewed });
  },
}));

/** Runs still needing the user's eyes: finished or errored, not yet viewed. */
/** PRs an agent has finished reviewing, keyed "owner/repo#number".
 * Lists use this to surface reviews nothing has been done with yet. */
export function useReviewedKeys(): Set<string> {
  const runs = useRunBoard((s) => s.runs);
  return useMemo(() => {
    const out = new Set<string>();
    for (const run of Object.values(runs)) {
      if (run.purpose === "pr review" && isTerminal(run)) {
        out.add(`${run.repo_slug}#${String(run.pr_number)}`);
      }
    }
    return out;
  }, [runs]);
}

export function unprocessedCount(runs: Record<string, AgentRun>, viewed: Set<string>): number {
  return Object.values(runs).filter((r) => isTerminal(r) && !viewed.has(r.run_id)).length;
}
