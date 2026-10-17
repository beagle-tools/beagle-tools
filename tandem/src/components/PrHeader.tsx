// PR title bar: state, branches, checks, reviews/approvers, labels.
// Read-only apart from the explicit GitHub actions it offers (approve,
// merge, ready-for-review).

import { Check, ExternalLink, FileText, RefreshCw, X } from "lucide-react";
import type { ReactElement, ReactNode, RefObject } from "react";
import { useRef, useState } from "react";

import { shortSha } from "../lib/format";
import { MergeControls } from "./MergeControls";
import { CommitsSection } from "./CommitsSection";
import { MergeStatus } from "./MergeStatus";
import { MergedBanner } from "./MergedBanner";
import { PrStateBadge } from "./PrStateBadge";
import { OpenInEditorButton } from "./OpenInEditor";
import { ApproveButton, CopyUrlButton, ReadyForReviewButton } from "./PrHeaderActions";
import { openExternal, prUrl } from "../lib/open";
import { MarkdownBody } from "./Markdown";
import type { PrDetail } from "../lib/types";
import { useAppStore } from "../state/store";
import { Button, checkTone, Pill, Spinner } from "./ui";

export function PrHeader({ detail }: { detail: PrDetail }): ReactElement {
  const pr = detail.pull_request;
  const refreshBundle = useAppStore((s) => s.refreshBundle);
  const viewer = useAppStore((s) => s.viewer);
  const syncing = useAppStore((s) => s.syncing);
  const busy = syncing[`pr:${pr.repo.owner}/${pr.repo.name}#${String(pr.number)}`] ?? false;

  const approvals = detail.reviews.filter((r) => r.verdict === "approved");
  const changesRequested = detail.reviews.filter((r) => r.verdict === "changes_requested");
  const failing = detail.checks.filter((c) => c.state === "failure");
  const pending = detail.checks.filter((c) => c.state === "pending");
  // Clicking a checks pill is a question ("which ones?") — answer it by
  // opening the full list and scrolling it into view, failures first.
  const [checksOpen, setChecksOpen] = useState(false);
  const checksRef = useRef<HTMLDetailsElement>(null);
  const openChecks = () => {
    setChecksOpen(true);
    setTimeout(() => {
      checksRef.current?.scrollIntoView({ behavior: "smooth", block: "nearest" });
    }, 0);
  };
  const orderedChecks = [
    ...failing,
    ...pending,
    ...detail.checks.filter((c) => c.state !== "failure" && c.state !== "pending"),
  ];

  return (
    <header className="border-b border-edge bg-panel px-4 py-3">
      <div className="flex items-start gap-3">
        <div className="min-w-0 flex-1">
          <h1 className="flex items-center gap-2 text-[15px] font-semibold text-cream">
            <PrStateBadge pr={pr} always />
            <span className="truncate">{pr.title}</span>
            <span className="shrink-0 font-normal text-muted">#{pr.number}</span>
          </h1>
          <div className="mt-1 flex flex-wrap items-center gap-2 text-xs text-muted">
            {/* Which repo this is — obvious from the list you came
                through, not at all obvious once you're reading a diff. */}
            <span className="font-mono text-cream">
              {pr.repo.owner}/{pr.repo.name}
            </span>
            <span>{pr.author.login}</span>
            <span className="rounded bg-panel-2 px-1.5 py-0.5 font-mono text-[11px]">
              {pr.head_ref} → {pr.base_ref}
            </span>
            <span className="font-mono text-[11px]">{shortSha(pr.head_sha)}</span>
            <span className="text-moss">+{pr.additions}</span>
            <span className="text-ember">−{pr.deletions}</span>
            <span>{pr.changed_files} files</span>
          </div>
        </div>
        <div className="flex flex-wrap items-center justify-end gap-2">
          {busy ? <Spinner label="syncing" /> : null}
          <Button
            onClick={() => {
              openExternal(prUrl(pr.repo, pr.number));
            }}
            title="open this PR on github.com"
          >
            <ExternalLink size={12} /> github
          </Button>
          <CopyUrlButton url={prUrl(pr.repo, pr.number)} />
          <OpenInEditorButton repo={`${pr.repo.owner}/${pr.repo.name}`} number={pr.number} />
          {pr.draft && viewer !== null && viewer === pr.author.login ? (
            <ReadyForReviewButton repo={`${pr.repo.owner}/${pr.repo.name}`} number={pr.number} />
          ) : null}
          <MergeControls pr={pr} />
          {viewer !== null && viewer === pr.author.login ? null : <ApproveButton />}
          <Button
            onClick={() => {
              void refreshBundle();
            }}
            title="Re-fetch from GitHub"
          >
            <RefreshCw size={12} /> refresh
          </Button>
        </div>
      </div>

      <PrPills
        detail={detail}
        failing={failing.length}
        pending={pending.length}
        approvals={approvals}
        changesRequested={changesRequested}
        openChecks={openChecks}
      />

      <MergedBanner pr={pr} />

      <MergeStatus
        state={pr.mergeable_state}
        approvals={approvals.map((r) => r.author.login)}
        changesRequested={changesRequested.map((r) => r.author.login)}
        failing={failing.map((c) => c.name)}
      />

      {pr.body.trim() ? (
        <details className="mt-2">
          <summary className="flex cursor-pointer items-center gap-1 text-[11px] text-muted hover:text-cream">
            <FileText size={10} /> description
          </summary>
          <div className="mt-2 max-h-72 overflow-y-auto rounded-lg border border-edge/60 bg-panel-2/40 p-3">
            <MarkdownBody text={pr.body} />
          </div>
        </details>
      ) : null}

      <CommitsSection repo={pr.repo} number={pr.number} />

      <AllChecks
        checks={orderedChecks}
        checksRef={checksRef}
        open={checksOpen}
        setOpen={setChecksOpen}
      />
    </header>
  );
}

/** A checks pill that opens the full check list when clicked. */
function ChecksPill(props: {
  tone: "ember" | "amber" | "moss";
  onClick: () => void;
  children: ReactNode;
}): ReactElement {
  return (
    <button
      type="button"
      onClick={props.onClick}
      title="show every check"
      className="rounded-full transition-transform hover:brightness-125 active:scale-95"
    >
      <Pill tone={props.tone}>{props.children}</Pill>
    </button>
  );
}

/** Checks summary, approvers, change requests, pending reviewers and
 * labels. `failing`/`pending` are check counts. */
function PrPills({
  detail,
  failing,
  pending,
  approvals,
  changesRequested,
  openChecks,
}: {
  detail: PrDetail;
  failing: number;
  pending: number;
  approvals: PrDetail["reviews"];
  changesRequested: PrDetail["reviews"];
  openChecks: () => void;
}): ReactElement {
  return (
    <div className="mt-2 flex flex-wrap items-center gap-1.5">
      {failing > 0 ? (
        <ChecksPill tone="ember" onClick={openChecks}>
          {failing} checks failing
        </ChecksPill>
      ) : null}
      {pending > 0 ? (
        <ChecksPill tone="amber" onClick={openChecks}>
          {pending} checks running
        </ChecksPill>
      ) : null}
      {failing === 0 && pending === 0 && detail.checks.length > 0 ? (
        <ChecksPill tone="moss" onClick={openChecks}>
          checks green
        </ChecksPill>
      ) : null}
      {approvals.map((r) => (
        <Pill key={r.author.login} tone="moss">
          <Check size={11} /> {r.author.login}
        </Pill>
      ))}
      {changesRequested.map((r) => (
        <Pill key={r.author.login} tone="ember">
          <X size={11} /> {r.author.login}
        </Pill>
      ))}
      {detail.pull_request.requested_reviewers.map((login) => (
        <Pill key={`await-${login}`} tone="amber">
          awaiting {login}
        </Pill>
      ))}
      {detail.pull_request.labels.map((label) => (
        <Pill key={label} tone="sky">
          {label}
        </Pill>
      ))}
    </div>
  );
}

/** Every check, failures first, in a collapsible list the checks pills
 * can open and scroll to. Nothing when the PR has no checks. */
function AllChecks({
  checks,
  checksRef,
  open,
  setOpen,
}: {
  checks: PrDetail["checks"];
  checksRef: RefObject<HTMLDetailsElement | null>;
  open: boolean;
  setOpen: (open: boolean) => void;
}): ReactElement | null {
  if (checks.length === 0) return null;
  return (
    <details
      ref={checksRef}
      open={open}
      onToggle={(e) => {
        setOpen(e.currentTarget.open);
      }}
      className="mt-2"
    >
      <summary className="cursor-pointer text-[11px] text-muted hover:text-cream">
        all checks ({checks.length})
      </summary>
      <div className="mt-1 flex flex-wrap gap-1.5">
        {checks.map((check) => (
          <Pill key={check.name} tone={checkTone(check.state)}>
            {check.name}
          </Pill>
        ))}
      </div>
    </details>
  );
}
