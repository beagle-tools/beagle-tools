// Agent-run notifications + jump-to highlighting. A notification is
// created when a run reaches a terminal state; clicking it navigates to
// the PR and flash-highlights the thread (or run row) it concerns.

import { create } from "zustand";

import type { AgentRun } from "../lib/types";

export interface RunNotification {
  id: number;
  runId: string;
  purpose: string;
  agent: string;
  repo: string;
  number: number;
  status: AgentRun["status"];
  targetCommentId: string | null;
  error: string | null;
  read: boolean;
}

interface NotificationStore {
  items: RunNotification[];
  add: (n: Omit<RunNotification, "id" | "read">) => void;
  markRead: (id: number) => void;
  markAllRead: () => void;
}

let nextId = 1;

export const useNotifications = create<NotificationStore>((set) => ({
  items: [],
  add: (n) => {
    set((s) => ({ items: [{ ...n, id: nextId++, read: false }, ...s.items].slice(0, 50) }));
  },
  markRead: (id) => {
    set((s) => ({ items: s.items.map((i) => (i.id === id ? { ...i, read: true } : i)) }));
  },
  markAllRead: () => {
    set((s) => ({ items: s.items.map((i) => ({ ...i, read: true })) }));
  },
}));

const TERMINAL = new Set(["succeeded", "failed", "cancelled", "timed_out"]);
const seen = new Set<string>();

/** Feed run updates here (from the tandem://run-updated listener). */
export function recordRunUpdate(run: AgentRun): void {
  if (!TERMINAL.has(run.status) || seen.has(run.run_id)) return;
  seen.add(run.run_id);
  useNotifications.getState().add({
    runId: run.run_id,
    purpose: run.purpose,
    agent: run.agent_name,
    repo: run.repo_slug,
    number: run.pr_number,
    status: run.status,
    targetCommentId: run.target_comment_id,
    error: run.error,
  });
}

// ---- jump-to highlight -------------------------------------------------

interface HighlightStore {
  commentId: string | null;
  runId: string | null;
  set: (target: { commentId?: string; runId?: string }) => void;
}

export const useHighlight = create<HighlightStore>((set) => ({
  commentId: null,
  runId: null,
  set: (target) => {
    set({ commentId: target.commentId ?? null, runId: target.runId ?? null });
    setTimeout(() => {
      set({ commentId: null, runId: null });
    }, 6000);
  },
}));
