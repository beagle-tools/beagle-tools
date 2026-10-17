// The PR row used by every list on the review-status page, plus the
// ready/open/draft grouping and the "approved by you" footer. Clicking
// a row opens that PR (any repo) in the review view.

import { CheckCircle2, RefreshCw } from "lucide-react";
import type { ReactElement, ReactNode } from "react";
import { useEffect, useRef } from "react";

import { relativeTime } from "../lib/format";
import { isReadyToMerge } from "../lib/ready";
import { EdgeStripes, useFailingChecksTip } from "./EdgeStripes";
import { sortPrs } from "../lib/sort";
import type { AgentRun, PullRequest } from "../lib/types";
import { useIsCursor, useKeyNav } from "../state/keyNav";
import { isWorking, useRunBoard } from "../state/runBoard";
import { useAppStore } from "../state/store";
import { PrStateBadge } from "./PrStateBadge";
import { Pill, Spinner } from "./ui";

/** Reviews already finished on this PR, so a list makes it obvious
 * whether an agent has looked at it before. Counts completed `pr
 * review` runs only — in-flight ones show as their own indicator. */
function useFinishedReviewCount(slug: string, number: number): number {
  return useRunBoard(
    (s) =>
      Object.values(s.runs).filter(
        (r: AgentRun) =>
          r.repo_slug === slug &&
          r.pr_number === number &&
          r.purpose === "pr review" &&
          !isWorking(r),
      ).length,
  );
}

const GROUPS: { label: string; filter: (pr: PullRequest) => boolean }[] = [
  { label: "ready to merge", filter: isReadyToMerge },
  { label: "open", filter: (pr) => !isReadyToMerge(pr) && !pr.draft },
  { label: "drafts", filter: (pr) => !isReadyToMerge(pr) && pr.draft },
];

/** Three labeled groups, each keeping the chosen sort order, separated
 * by a divider: ready to merge, then open-not-approved, then drafts
 * last — so it's obvious at a glance which PRs are still drafts.
 * `renderAside`, when given, adds a per-row control (e.g. quick-start-
 * review) on the right of each row. */
export function ReadyGroupedRows({
  prs,
  renderAside,
  unacted,
}: {
  prs: PullRequest[];
  renderAside?: (pr: PullRequest) => ReactNode;
  /** PRs an agent reviewed that nothing has been done with — they lead,
   * above the usual ready/open/draft split. */
  unacted?: (pr: PullRequest) => boolean;
}): ReactElement {
  const leading = unacted ? prs.filter(unacted) : [];
  const remaining = unacted ? prs.filter((pr) => !unacted(pr)) : prs;
  const groups = [
    { label: "reviewed · nothing done with it yet", prs: leading },
    ...GROUPS.map((g) => ({ label: g.label, prs: remaining.filter(g.filter) })),
  ];
  const visible = groups.filter((g) => g.prs.length > 0);
  // Publish the on-screen order (groups flattened) for j/k navigation.
  const flat = groups.flatMap((g) => g.prs);
  useEffect(() => {
    useKeyNav
      .getState()
      .setList(flat.map((pr) => ({ slug: `${pr.repo.owner}/${pr.repo.name}`, number: pr.number })));
  }, [flat]);
  return (
    <>
      {visible.map(({ prs: group, label }, i) => (
        <div key={label} className="space-y-1">
          {i > 0 ? <div className="mx-3 my-2 border-t border-edge/70" /> : null}
          <div
            className={`px-1 text-[10px] font-medium uppercase tracking-wide ${
              label.startsWith("reviewed") ? "text-amber" : "text-muted"
            }`}
          >
            {label} ({group.length})
          </div>
          {group.map((pr) => (
            <InboxRow key={rowKey(pr)} pr={pr} aside={renderAside?.(pr)} />
          ))}
        </div>
      ))}
    </>
  );
}

export function rowKey(pr: PullRequest): string {
  return `${pr.repo.owner}/${pr.repo.name}#${String(pr.number)}`;
}

export function InboxRow({
  pr,
  aside,
  note,
}: {
  pr: PullRequest;
  aside?: ReactNode;
  /** Short reason this row is in the list, when it isn't obvious. */
  note?: string;
}): ReactElement {
  const openPr = useAppStore((s) => s.openPr);
  const selectedRepo = useAppStore((s) => s.selectedRepo);
  const selectedPr = useAppStore((s) => s.selectedPr);
  const slug = `${pr.repo.owner}/${pr.repo.name}`;
  const active = selectedRepo === slug && selectedPr === pr.number;
  const { onMouseEnter, onMouseLeave, tipEl } = useFailingChecksTip(pr);
  const isCursor = useIsCursor(slug, pr.number);
  const reviews = useFinishedReviewCount(slug, pr.number);
  const ref = useRef<HTMLButtonElement | null>(null);
  useEffect(() => {
    if (isCursor) ref.current?.scrollIntoView({ block: "nearest" });
  }, [isCursor]);
  return (
    <div className="flex items-center gap-1.5">
      <button
        ref={ref}
        type="button"
        onClick={() => {
          void openPr(slug, pr.number);
        }}
        onMouseEnter={onMouseEnter}
        onMouseLeave={onMouseLeave}
        className={`animate-fade-up relative block min-w-0 flex-1 overflow-hidden rounded-lg border px-3 py-2 text-left transition-all ${
          active
            ? "border-sky/40 bg-panel-2 shadow-sm"
            : isCursor
              ? "border-sky/60 bg-panel-2/60 ring-1 ring-sky/30"
              : "border-transparent hover:border-edge hover:bg-panel-2/60"
        }`}
      >
        <EdgeStripes pr={pr} />
        {tipEl}
        <div className="mb-0.5 flex items-center gap-2 text-[10px] text-muted">
          <span className="truncate font-mono">{slug}</span>
          <span className="ml-auto shrink-0">{relativeTime(pr.updated_at)}</span>
        </div>
        <div className="flex items-center gap-1.5 text-[12px] text-cream">
          <span className="shrink-0 font-medium text-amber">#{pr.number}</span>
          {/* Badge before the title so a long title can't push the one
              thing that says "this isn't ready" off the row. */}
          <PrStateBadge pr={pr} />
          {note === undefined ? null : <Pill tone="sky">{note}</Pill>}
          <span className="truncate">{pr.title}</span>
        </div>
        <div className="mt-0.5 flex items-center gap-1.5 text-[10px] text-muted">
          {pr.author.login}
          {reviews > 0 ? (
            <span className="inline-flex items-center gap-1 rounded-full bg-moss/15 px-1.5 text-[9px] font-medium text-moss">
              <CheckCircle2 size={9} />
              {reviews} bot review{reviews === 1 ? "" : "s"}
            </span>
          ) : null}
        </div>
      </button>
      {aside ? <div className="shrink-0">{aside}</div> : null}
    </div>
  );
}

/** Bottom of the requested tab: PRs you already approved that are
 * still open — handy for "did that ever merge?" follow-ups. */
export function ApprovedFooter(): ReactElement {
  const raw = useAppStore((s) => s.inbox.approved);
  const prSort = useAppStore((s) => s.prSort);
  const loadInbox = useAppStore((s) => s.loadInbox);
  const prs = raw === undefined ? undefined : sortPrs(raw, prSort);

  useEffect(() => {
    void loadInbox("approved");
  }, [loadInbox]);

  return (
    <details open className="border-t border-edge/60 px-2 py-2">
      <summary className="flex cursor-pointer items-center gap-1.5 px-1 py-1 text-[11px] uppercase tracking-wide text-moss hover:text-cream">
        <CheckCircle2 size={11} /> approved by you · still open ({prs?.length ?? "…"})
        <button
          type="button"
          title="refresh"
          onClick={(e) => {
            e.preventDefault();
            void loadInbox("approved", true);
          }}
          className="ml-auto inline-flex size-5 items-center justify-center rounded text-muted hover:bg-panel-2 hover:text-cream"
        >
          <RefreshCw size={10} />
        </button>
      </summary>
      <div className="mt-1 space-y-1 px-0">
        {prs === undefined ? (
          <div className="px-3 py-3">
            <Spinner label="checking your approvals…" />
          </div>
        ) : prs.length === 0 ? (
          <div className="px-3 py-3 text-center text-xs text-muted">none pending merge</div>
        ) : (
          prs.map((pr) => <InboxRow key={rowKey(pr)} pr={pr} />)
        )}
      </div>
    </details>
  );
}
