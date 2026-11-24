// The agents dashboard — a dedicated page listing every agent run
// across all repos, sectioned org → repo → PR → run. Working runs
// spin, finished runs wait for a look, errored runs glow ember; once
// the user jumps to a terminal run it is processed and drops off.

import { Bot, CheckCheck, GitPullRequest } from "lucide-react";
import { useEffect, useState } from "react";
import type { ReactElement } from "react";

import { ipc } from "../lib/ipc";
import type { AgentRun } from "../lib/types";
import { useHighlight } from "../state/notifications";
import { isError, isTerminal, isWorking, unprocessedCount, useRunBoard } from "../state/runBoard";
import { useThreadPane } from "./ThreadPane";
import { useAppStore } from "../state/store";
import { pushGithubError, pushInfo } from "../state/toasts";
import { RunRow } from "./AgentRunRow";
import { Button, EmptyState, IconButton } from "./ui";

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
