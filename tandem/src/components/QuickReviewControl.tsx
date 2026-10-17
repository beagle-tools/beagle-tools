// Per-row "start review" control: pick an agent and kick off a review
// right from a PR list, without opening the PR first. Shares the same
// last-picked-agent memory as the review page's own agent picker.

import { Loader2, Play } from "lucide-react";
import { useState } from "react";
import type { ReactElement } from "react";

import type { PullRequest } from "../lib/types";
import { loadAgent, saveAgent } from "../state/persist";
import { isWorking, useRunBoard } from "../state/runBoard";
import { useAppStore } from "../state/store";

export function QuickReviewControl({ pr }: { pr: PullRequest }): ReactElement | null {
  const specs = useAppStore((s) => s.agentSpecs);
  const quickStartReview = useAppStore((s) => s.quickStartReview);
  const openPr = useAppStore((s) => s.openPr);
  const [agentName, setAgentName] = useState(loadAgent);
  const [starting, setStarting] = useState(false);
  const selected = agentName || (specs[0]?.name ?? "");
  const slug = `${pr.repo.owner}/${pr.repo.name}`;
  // Any in-flight run on this PR, wherever it was started from — the
  // run board is account-wide and updated live by tandem://run-updated.
  const activeRun = useRunBoard((s) =>
    Object.values(s.runs).find(
      (r) => r.repo_slug === slug && r.pr_number === pr.number && isWorking(r),
    ),
  );

  // Checked before the no-agents bail-out: a run can still be in flight
  // for this PR even if every agent spec was deleted since.
  if (activeRun ?? starting) {
    const label = (activeRun?.purpose ?? "pr review") === "pr review" ? "reviewing" : "working";
    return (
      <button
        type="button"
        onClick={() => {
          void openPr(slug, pr.number);
        }}
        title={`${activeRun?.agent_name ?? selected} is ${label} — open the PR to watch`}
        className="inline-flex h-7 shrink-0 items-center gap-1.5 rounded-lg border border-sky/40 bg-sky/10 px-2 text-[11px] text-sky transition-colors hover:bg-sky/20"
      >
        <Loader2 size={11} className="animate-spin" />
        {label}…
      </button>
    );
  }

  if (specs.length === 0) return null;

  return (
    <div className="flex items-stretch overflow-hidden rounded-lg border border-edge bg-panel-2 transition-colors focus-within:border-sky hover:border-edge">
      <select
        value={selected}
        onChange={(e) => {
          setAgentName(e.target.value);
          saveAgent(e.target.value);
        }}
        title="agent for this review"
        className="h-7 max-w-28 cursor-pointer border-0 bg-transparent pl-2 pr-1 text-[11px] text-muted outline-none transition-colors hover:text-cream"
      >
        {specs.map((spec) => (
          <option key={spec.name} value={spec.name}>
            {spec.name}
          </option>
        ))}
      </select>
      <span className="w-px bg-edge" />
      <button
        type="button"
        disabled={!selected}
        title={`start a ${selected} review of #${String(pr.number)}`}
        onClick={() => {
          setStarting(true);
          void quickStartReview(slug, pr.number, selected).finally(() => {
            setStarting(false);
          });
        }}
        className="inline-flex h-7 w-7 items-center justify-center text-sky transition-all hover:bg-sky/15 active:scale-95 disabled:opacity-40"
      >
        <Play size={12} />
      </button>
    </div>
  );
}
