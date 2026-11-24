// One agent run's row on the agents dashboard: status, agent, purpose,
// and stop / rerun / delete. Split from AgentsDashboard.tsx.

import {
  Ban,
  Check,
  GitMerge,
  Loader2,
  MessageSquare,
  RotateCcw,
  ScanSearch,
  Square,
  Trash2,
  X,
} from "lucide-react";
import { useState } from "react";
import type { ReactElement } from "react";

import { relativeTime } from "../lib/format";
import type { AgentRun } from "../lib/types";
import { isError, isTerminal, isWorking } from "../state/runBoard";
import { IconButton } from "./ui";

const PURPOSE_LABEL: Record<string, string> = {
  "pr review": "PR review",
  "thread reply": "thread reply",
  "conflict analysis": "conflict analysis",
};

function PurposeIcon({ purpose }: { purpose: string }) {
  if (purpose === "thread reply") return <MessageSquare size={11} />;
  if (purpose === "conflict analysis") return <GitMerge size={11} />;
  return <ScanSearch size={11} />;
}

function StatusIcon({ run }: { run: AgentRun }) {
  if (isWorking(run)) return <Loader2 size={13} className="animate-spin text-sky" />;
  if (run.status === "succeeded") return <Check size={13} className="text-moss" />;
  if (run.status === "cancelled") return <Ban size={13} className="text-muted" />;
  return <X size={13} className="text-ember" />;
}

export function RunRow({
  run,
  processed,
  onJump,
  onStop,
  onRerun,
  onDelete,
}: {
  run: AgentRun;
  processed: boolean;
  onJump: () => void;
  onStop: () => void;
  onRerun: () => void;
  onDelete: () => void;
}): ReactElement {
  const failed = isError(run);
  const [confirmingDelete, setConfirmingDelete] = useState(false);
  return (
    <div
      className={`group flex w-full items-center gap-2 px-3 py-2 transition-colors hover:bg-panel-2 ${
        processed && isTerminal(run) ? "opacity-50" : ""
      } ${failed && !processed ? "bg-ember/5" : ""}`}
    >
      <button
        type="button"
        onClick={onJump}
        className="flex min-w-0 flex-1 items-center gap-2 text-left"
        title={
          failed ? "open the PR and inspect the run log" : "open the PR and jump to the result"
        }
      >
        <StatusIcon run={run} />
        <span className="min-w-0 flex-1">
          <span className="flex items-center gap-1.5 text-xs text-cream">
            {run.agent_name}
            <span className="flex items-center gap-1 text-muted">
              <PurposeIcon purpose={run.purpose} />
              {PURPOSE_LABEL[run.purpose] ?? run.purpose}
            </span>
          </span>
          <span
            className={`mt-0.5 block truncate text-[10px] ${failed ? "text-ember" : "text-muted"}`}
          >
            {run.status.replace("_", " ")}
            {run.comment_count > 0 ? ` · ${String(run.comment_count)} comments` : ""}
            {run.error !== null && run.error ? ` — ${run.error}` : ""}
            {failed && !run.error ? " · click to view and fix" : ""}
          </span>
        </span>
      </button>
      <span className="flex shrink-0 items-center gap-0.5">
        {isWorking(run) ? (
          <button
            type="button"
            onClick={onStop}
            title="stop this run"
            className="inline-flex size-7 items-center justify-center rounded-md text-ember/80 transition-colors hover:bg-ember/10 hover:text-ember"
          >
            <Square size={11} fill="currentColor" />
          </button>
        ) : (
          <>
            <IconButton onClick={onRerun} title="run this agent again">
              <RotateCcw size={12} />
            </IconButton>
            {confirmingDelete ? (
              <button
                type="button"
                onClick={onDelete}
                onMouseLeave={() => {
                  setConfirmingDelete(false);
                }}
                title="permanently delete this run and its local comments"
                className="inline-flex h-7 items-center gap-1 rounded-lg bg-ember/15 px-1.5 text-[10px] font-medium text-ember"
              >
                <Trash2 size={12} /> sure?
              </button>
            ) : (
              <IconButton
                onClick={() => {
                  setConfirmingDelete(true);
                }}
                title="delete this run and its local conversation"
              >
                <Trash2 size={12} />
              </IconButton>
            )}
          </>
        )}
      </span>
      <span className="shrink-0 text-[10px] text-muted">{relativeTime(run.started_at)}</span>
    </div>
  );
}
