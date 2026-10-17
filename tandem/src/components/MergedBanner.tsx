// A merged PR is finished work. Say so loudly (plum, nothing else in the
// app is), and offer the one move that makes sense next: go pick another
// PR to review. Leaving the page also wipes everything Tandem cached for
// it — see `forgetIfMerged` in the store.

import type { ReactElement } from "react";
import { GitMerge } from "lucide-react";

import { useKeyNav } from "../state/keyNav";
import { useAppStore } from "../state/store";
import type { PullRequest } from "../lib/types";
import { Button } from "./ui";

export function MergedBanner({ pr }: { pr: PullRequest }): ReactElement | null {
  const goHome = useAppStore((s) => s.goHome);
  const setTab = useKeyNav((s) => s.setTab);
  if (pr.state !== "merged") return null;

  return (
    <div className="mt-2 flex flex-wrap items-center gap-2 rounded-lg border border-plum/40 bg-plum/10 px-3 py-2 text-[11px]">
      <GitMerge size={12} className="text-plum" />
      <span className="font-medium text-plum">merged</span>
      <span className="text-muted">
        nothing left to review — Tandem drops its local copy when you leave
      </span>
      <span className="ml-auto">
        <Button
          kind="primary"
          title="back to the review requests on the review status page"
          onClick={() => {
            setTab("requested");
            goHome();
          }}
        >
          review another PR
        </Button>
      </span>
    </div>
  );
}
