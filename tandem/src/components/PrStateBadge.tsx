// One way of saying what state a PR is in, used by every list and by
// the PR header. Colors follow GitHub's own vocabulary so the meaning
// carries over: grey-dashed draft, green open, plum merged, red closed.

import type { ReactElement } from "react";
import { GitMerge, GitPullRequest, GitPullRequestClosed, GitPullRequestDraft } from "lucide-react";

import type { PullRequest } from "../lib/types";

interface Look {
  label: string;
  icon: typeof GitPullRequest;
  className: string;
}

function look(pr: PullRequest): Look {
  if (pr.state === "merged") {
    return { label: "merged", icon: GitMerge, className: "border-plum/40 bg-plum/15 text-plum" };
  }
  if (pr.state === "closed") {
    return {
      label: "closed",
      icon: GitPullRequestClosed,
      className: "border-ember/40 bg-ember/15 text-ember",
    };
  }
  if (pr.draft) {
    // Dashed on purpose — "not finished" reads before the word does.
    return {
      label: "draft",
      icon: GitPullRequestDraft,
      className: "border-dashed border-muted/70 bg-panel-2 text-muted",
    };
  }
  return { label: "open", icon: GitPullRequest, className: "border-moss/40 bg-moss/15 text-moss" };
}

/** By default only states worth interrupting for show up (draft, merged,
 * closed) — a plain open PR in a list of open PRs needs no badge. Pass
 * `always` where the state should be stated outright, like the header of
 * the PR you're reading. */
export function PrStateBadge({
  pr,
  always = false,
}: {
  pr: PullRequest;
  always?: boolean;
}): ReactElement | null {
  if (!always && pr.state === "open" && !pr.draft) return null;
  const { label, icon: Icon, className } = look(pr);
  return (
    <span
      className={`inline-flex shrink-0 items-center gap-1 whitespace-nowrap rounded-full border px-1.5 py-[2px] text-[10px] font-semibold leading-none ${className}`}
    >
      <Icon size={10} />
      {label}
    </span>
  );
}
