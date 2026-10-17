// The agents dashboard — a dedicated page listing every agent run
// across all repos, sectioned org → repo → PR → run. Working runs
// spin, finished runs wait for a look, errored runs glow ember; once
// the user jumps to a terminal run it is processed and drops off.

import {
  Ban,
  Bot,
  Check,
  CheckCheck,
  GitMerge,
  GitPullRequest,
  Loader2,
  MessageSquare,
  RotateCcw,
  ScanSearch,
  Square,
  Trash2,
  X,
} from "lucide-react";
import { useEffect, useState } from "react";
import type { ReactElement } from "react";

import { ipc } from "../lib/ipc";
import { relativeTime } from "../lib/format";
import type { AgentRun } from "../lib/types";
import { useHighlight } from "../state/notifications";
import { isError, isTerminal, isWorking, unprocessedCount, useRunBoard } from "../state/runBoard";
import { useThreadPane } from "./ThreadPane";
import { useAppStore } from "../state/store";
import { pushGithubError, pushInfo } from "../state/toasts";
import { Button, EmptyState, IconButton } from "./ui";

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

interface PrGroup {
  number: number;
  runs: AgentRun[];
}
interface RepoGroup {
  slug: string;
  prs: PrGroup[];
}
interface OrgGroup {
  org: string;
  repos: RepoGroup[];
}

function groupRuns(runs: AgentRun[]): OrgGroup[] {
  const orgs = new Map<string, Map<string, Map<number, AgentRun[]>>>();
  for (const run of runs) {
    const org = run.repo_slug.split("/")[0] ?? run.repo_slug;
    const repos = orgs.get(org) ?? new Map<string, Map<number, AgentRun[]>>();
    const prs = repos.get(run.repo_slug) ?? new Map<number, AgentRun[]>();
    const list = prs.get(run.pr_number) ?? [];
    list.push(run);
    prs.set(run.pr_number, list);
    repos.set(run.repo_slug, prs);
    orgs.set(org, repos);
  }
  return [...orgs.entries()]
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([org, repos]) => ({
      org,
      repos: [...repos.entries()]
        .sort(([a], [b]) => a.localeCompare(b))
        .map(([slug, prs]) => ({
          slug,
          prs: [...prs.entries()]
            .sort(([a], [b]) => b - a)
            .map(([number, list]) => ({
              number,
              runs: list.sort((x, y) => y.started_at.localeCompare(x.started_at)),
            })),
        })),
    }));
}

function SummaryChip({ tone, label, count }: { tone: string; label: string; count: number }) {
  return (
    <span className={`flex items-center gap-1.5 rounded-full px-2.5 py-1 text-[11px] ${tone}`}>
      <span className="font-semibold">{count}</span> {label}
    </span>
  );
}

function fail(e: unknown): void {
  pushGithubError(e instanceof Error ? e.message : String(e));
}

function stop(run: AgentRun): void {
  ipc.cancelAgentRun(run.run_id).catch(fail);
}

// Relaunch with the same agent + purpose; thread replies re-run on
// the same thread, everything else starts a fresh run of its kind.
function rerun(run: AgentRun): void {
  const relaunch =
    run.purpose === "conflict analysis"
      ? ipc.startConflictResolution(run.agent_name, run.repo_slug, run.pr_number)
      : run.purpose === "thread reply" && run.target_comment_id !== null
        ? ipc.mentionAgent(run.agent_name, run.target_comment_id)
        : ipc.startAgentReview(run.agent_name, run.repo_slug, run.pr_number);
  relaunch
    .then(() => {
      pushInfo(`${run.agent_name} relaunched`);
    })
    .catch(fail);
}

function destroy(run: AgentRun, removeRun: (runId: string) => void): void {
  ipc
    .deleteAgentRun(run.run_id)
    .then(() => {
      removeRun(run.run_id);
    })
    .catch(fail);
}

export function AgentsDashboard(): ReactElement {
  const runsMap = useRunBoard((s) => s.runs);
  const viewed = useRunBoard((s) => s.viewed);
  const markViewed = useRunBoard((s) => s.markViewed);
  const markAllViewed = useRunBoard((s) => s.markAllViewed);
  const ingestMany = useRunBoard((s) => s.ingestMany);
  const openPr = useAppStore((s) => s.openPr);
  const setView = useAppStore((s) => s.setView);
  const setHighlight = useHighlight((s) => s.set);
  const openThread = useThreadPane((s) => s.open);
  const [showProcessed, setShowProcessed] = useState(false);

  useEffect(() => {
    ipc.listAllAgentRuns().then(ingestMany).catch(console.warn);
  }, [ingestMany]);

  const all = Object.values(runsMap);
  const working = all.filter(isWorking).length;
  const errors = all.filter((r) => isError(r) && !viewed.has(r.run_id)).length;
  const finished = all.filter((r) => isTerminal(r) && !isError(r) && !viewed.has(r.run_id)).length;
  const processed = all.filter((r) => isTerminal(r) && viewed.has(r.run_id)).length;

  const groups = groupRuns(
    all.filter((r) => showProcessed || !(isTerminal(r) && viewed.has(r.run_id))),
  );

  const jump = (run: AgentRun) => {
    if (isTerminal(run)) markViewed(run.run_id);
    // A finished thread reply is something you read and answer — show
    // it where you are rather than navigating away from your place.
    if (run.purpose === "thread reply" && run.target_comment_id !== null && isTerminal(run)) {
      openThread({
        repo: run.repo_slug,
        number: run.pr_number,
        commentId: run.target_comment_id,
      });
      return;
    }
    setView("review");
    void openPr(run.repo_slug, run.pr_number);
    setHighlight(
      run.target_comment_id !== null ? { commentId: run.target_comment_id } : { runId: run.run_id },
    );
  };

  const removeRun = useRunBoard((s) => s.remove);

  return (
    <div className="mx-auto w-full max-w-3xl px-6 py-5">
      <div className="flex flex-wrap items-center gap-2">
        <h1 className="mr-2 flex items-center gap-2 text-sm font-semibold text-cream">
          <Bot size={16} /> agent runs
        </h1>
        <SummaryChip tone="bg-sky/10 text-sky" label="working" count={working} />
        <SummaryChip tone="bg-amber/10 text-amber" label="to review" count={finished} />
        <SummaryChip tone="bg-ember/10 text-ember" label="errors" count={errors} />
        <span className="ml-auto flex items-center gap-1.5">
          {processed > 0 ? (
            <Button
              onClick={() => {
                setShowProcessed((v) => !v);
              }}
            >
              {showProcessed ? "hide processed" : `show ${String(processed)} processed`}
            </Button>
          ) : null}
          {finished + errors > 0 ? (
            <Button onClick={markAllViewed} title="mark every finished run as processed">
              <CheckCheck size={12} /> mark all processed
            </Button>
          ) : null}
        </span>
      </div>

      {groups.length === 0 ? (
        <EmptyState
          title="all caught up"
          hint="no agent runs need your attention — launch a review from any PR's agents panel"
        />
      ) : (
        groups.map((org) => (
          <section key={org.org} className="mt-5">
            <h2 className="text-[11px] font-medium uppercase tracking-wide text-muted">
              {org.org}
            </h2>
            {org.repos.map((repo) => (
              <div
                key={repo.slug}
                className="mt-2 overflow-hidden rounded-xl border border-edge bg-panel"
              >
                <div className="border-b border-edge px-3 py-2 font-mono text-xs text-cream">
                  {repo.slug}
                </div>
                {repo.prs.map((pr) => (
                  <div key={pr.number} className="border-b border-edge/40 last:border-b-0">
                    <div className="flex items-center gap-1.5 px-3 pb-1 pt-2 font-mono text-[11px] text-muted">
                      <GitPullRequest size={11} /> #{pr.number}
                    </div>
                    {pr.runs.map((run) => (
                      <RunRow
                        key={run.run_id}
                        run={run}
                        processed={viewed.has(run.run_id)}
                        onJump={() => {
                          jump(run);
                        }}
                        onStop={() => {
                          stop(run);
                        }}
                        onRerun={() => {
                          rerun(run);
                        }}
                        onDelete={() => {
                          destroy(run, removeRun);
                        }}
                      />
                    ))}
                  </div>
                ))}
              </div>
            ))}
          </section>
        ))
      )}
    </div>
  );
}

function RunRow({
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
}) {
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

/** Header entry: opens the dashboard, badges live run activity. */
export function AgentsMenuButton(): ReactElement {
  const runs = useRunBoard((s) => s.runs);
  const viewed = useRunBoard((s) => s.viewed);
  const view = useAppStore((s) => s.view);
  const setView = useAppStore((s) => s.setView);

  const all = Object.values(runs);
  const working = all.filter(isWorking).length;
  const attention = unprocessedCount(runs, viewed);
  const hasErrors = all.some((r) => isError(r) && !viewed.has(r.run_id));
  const count = working + attention;
  const badgeTone = hasErrors ? "bg-ember" : working > 0 ? "bg-sky" : "bg-amber";

  return (
    <IconButton
      onClick={() => {
        setView(view === "agents" ? "review" : "agents");
      }}
      title="agent runs dashboard"
      active={view === "agents"}
    >
      <span className="relative">
        <Bot size={15} />
        {count > 0 ? (
          <span
            className={`absolute -right-1.5 -top-1.5 flex size-3.5 items-center justify-center rounded-full text-[8px] font-bold text-ground ${badgeTone} ${working > 0 ? "animate-pulse" : ""}`}
          >
            {count > 9 ? "9+" : count}
          </span>
        ) : null}
      </span>
    </IconButton>
  );
}
