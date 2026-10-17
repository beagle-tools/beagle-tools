// One line of truth about merge readiness: who approved, and exactly
// what's blocking when GitHub says the PR can't merge yet. Lives apart
// from PrHeader to keep both files inside the 400-line rule.

import type { ReactElement } from "react";
import { GitMerge, OctagonAlert } from "lucide-react";

import { Pill } from "./ui";

export function MergeStatus(props: {
  state: string | null;
  approvals: string[];
  changesRequested: string[];
  failing: string[];
}): ReactElement | null {
  const { state } = props;
  const pill = (() => {
    switch (state ?? "") {
      case "clean":
      case "has_hooks":
        return { label: "ready to merge", tone: "moss" as const };
      case "unstable":
        return { label: "checks pending", tone: "amber" as const };
      case "behind":
        return { label: "behind base", tone: "amber" as const };
      case "dirty":
        return { label: "merge conflicts", tone: "ember" as const };
      case "blocked":
        return { label: "blocked", tone: "ember" as const };
      case "draft":
        return { label: "draft", tone: "muted" as const };
      default:
        return null;
    }
  })();

  const blockers: string[] = [];
  if (props.changesRequested.length > 0) {
    blockers.push(`changes requested by ${props.changesRequested.join(", ")}`);
  }
  if (props.failing.length > 0) {
    const names = props.failing.slice(0, 3).join(", ");
    const more = props.failing.length > 3 ? ` +${String(props.failing.length - 3)} more` : "";
    blockers.push(`failing checks: ${names}${more}`);
  }
  if (state === "dirty") blockers.push("merge conflicts with the base branch");
  if (state === "behind") blockers.push("branch is behind the base branch");
  if (state === "blocked" && blockers.length === 0) {
    blockers.push("required approvals or checks not yet satisfied");
  }

  if (!pill && props.approvals.length === 0 && blockers.length === 0) return null;

  return (
    <div className="mt-2 rounded-lg border border-edge/60 bg-panel-2/40 px-3 py-2 text-[11px]">
      <div className="flex flex-wrap items-center gap-2">
        <GitMerge size={12} className="text-muted" />
        {pill ? <Pill tone={pill.tone}>{pill.label}</Pill> : null}
        <span className="text-muted">
          {props.approvals.length > 0
            ? `approved by ${props.approvals.join(", ")}`
            : "no approvals yet"}
        </span>
      </div>
      {blockers.length > 0 ? (
        <div className="mt-1 space-y-0.5">
          {blockers.map((reason) => (
            <div key={reason} className="flex items-center gap-1.5 text-ember">
              <OctagonAlert size={11} className="shrink-0" />
              {reason}
            </div>
          ))}
        </div>
      ) : null}
    </div>
  );
}
