// Stacked status stripes on the left edge of PR cards — green (ready),
// red (checks failing), amber (unresolved review threads) — plus a
// hover tip listing exactly which checks are failing.

import { useState } from "react";
import type { ReactElement } from "react";

import { ipc } from "../lib/ipc";
import { checksFailing, hasMergeConflicts, isReadyToMerge } from "../lib/ready";
import type { PullRequest } from "../lib/types";

export function EdgeStripes({ pr }: { pr: PullRequest }): ReactElement | null {
  const stripes = [
    isReadyToMerge(pr) ? "bg-moss" : null,
    checksFailing(pr) || hasMergeConflicts(pr) ? "bg-ember" : null,
    pr.unresolved_threads > 0 ? "bg-amber" : null,
  ].filter((c): c is string => c !== null);
  if (stripes.length === 0) return null;
  return (
    <span className="pointer-events-none absolute inset-y-0 left-0 flex">
      {stripes.map((cls) => (
        <span key={cls} className={`h-full w-[3px] ${cls}`} />
      ))}
    </span>
  );
}

const checkCache = new Map<string, string[]>();

interface Tip {
  x: number;
  y: number;
  failing: string[] | null;
}

/** What {@link useFailingChecksTip} hands a PR row. */
export interface FailingChecksTip {
  onMouseEnter: (e: React.MouseEvent) => void;
  onMouseLeave: () => void;
  tipEl: ReactElement | null;
}

/** Row handlers + floating box explaining every colored stripe: ready,
 * failing checks (with names), pending checks, review state, and
 * unresolved comment threads. */
export function useFailingChecksTip(pr: PullRequest): FailingChecksTip {
  const [tip, setTip] = useState<Tip | null>(null);
  const slug = `${pr.repo.owner}/${pr.repo.name}`;
  const key = `${slug}#${String(pr.number)}@${pr.head_sha}`;
  const failing = checksFailing(pr);
  const conflicted = hasMergeConflicts(pr);
  const hasStripes = failing || conflicted || isReadyToMerge(pr) || pr.unresolved_threads > 0;

  const onMouseEnter = (e: React.MouseEvent) => {
    if (!hasStripes) return;
    const cached = checkCache.get(key);
    setTip({ x: e.clientX, y: e.clientY, failing: cached ?? null });
    if (failing && !cached) {
      ipc
        .listFailingChecks(slug, pr.number)
        .then((names) => {
          checkCache.set(key, names);
          setTip((t) => (t ? { ...t, failing: names } : t));
        })
        .catch(console.warn);
    }
  };
  const onMouseLeave = () => {
    setTip(null);
  };

  const lines: { text: string; cls: string }[] = [];
  if (isReadyToMerge(pr)) {
    lines.push({ text: "ready to merge", cls: "text-moss" });
  } else {
    if (conflicted) lines.push({ text: "merge conflicts with the base branch", cls: "text-ember" });
    if (pr.review_decision === "CHANGES_REQUESTED")
      lines.push({ text: "changes requested", cls: "text-ember" });
    else if (pr.review_decision === "REVIEW_REQUIRED")
      lines.push({ text: "review required — not approved yet", cls: "text-amber" });
    if (pr.checks_state === "PENDING")
      lines.push({ text: "checks still running", cls: "text-amber" });
  }
  if (pr.unresolved_threads > 0) {
    lines.push({
      text: `${String(pr.unresolved_threads)} unresolved comment thread${
        pr.unresolved_threads === 1 ? "" : "s"
      }`,
      cls: "text-amber",
    });
  }

  const tipEl = tip ? (
    <div
      className="animate-fade-in fixed z-[90] max-w-72 rounded-lg border border-edge bg-panel px-3 py-2 text-left shadow-2xl"
      style={{ left: tip.x + 14, top: tip.y + 10 }}
    >
      <div className="mb-1 text-[10px] font-medium uppercase tracking-wide text-muted">status</div>
      <ul className="space-y-0.5 text-[11px]">
        {lines.map((line) => (
          <li key={line.text} className={line.cls}>
            {line.text}
          </li>
        ))}
        {failing ? (
          tip.failing === null ? (
            <li className="text-muted">loading failing checks…</li>
          ) : (
            tip.failing.slice(0, 8).map((name) => (
              <li key={name} className="truncate text-ember">
                ✗ {name}
              </li>
            ))
          )
        ) : null}
        {failing && tip.failing !== null && tip.failing.length > 8 ? (
          <li className="text-muted">+{tip.failing.length - 8} more</li>
        ) : null}
      </ul>
    </div>
  ) : null;

  return { onMouseEnter, onMouseLeave, tipEl };
}
