// The last look before a conflict resolution leaves the machine: the
// diff the push will commit for the conflicted files, and the button
// that pushes it. Nothing else the agent touched is committed.

import { GitMerge, Loader2, Upload } from "lucide-react";
import type { ReactElement } from "react";

import { Modal } from "./ui";

export function ConflictPushModal({
  diff,
  pushing,
  onPush,
  onClose,
}: {
  /** `git diff HEAD` over the conflicted paths; null while loading. */
  diff: string | null;
  pushing: boolean;
  onPush: () => void;
  onClose: () => void;
}): ReactElement {
  return (
    <Modal
      title={
        <span className="inline-flex items-center gap-1.5">
          <GitMerge size={12} /> push the resolution?
        </span>
      }
      onClose={onClose}
    >
      <div className="flex h-full flex-col gap-2">
        <p className="text-[11px] text-muted">
          This commits the merge with exactly these changes to the conflicted files, then pushes it
          to the PR branch. Anything else in the checkout stays out of the commit.
        </p>
        <pre
          aria-label="resolution diff"
          className="min-h-0 flex-1 overflow-auto rounded-md border border-edge bg-panel-2 p-2 font-mono text-[11px] leading-snug text-cream"
        >
          {diff === null ? "loading the diff…" : diff === "" ? "(no changes)" : diff}
        </pre>
        <div className="flex justify-end gap-1.5">
          <button
            type="button"
            onClick={onClose}
            className="rounded-md px-2 py-1 text-[11px] text-muted transition-colors hover:text-cream"
          >
            cancel
          </button>
          <button
            type="button"
            onClick={onPush}
            disabled={pushing || diff === null}
            className="inline-flex items-center gap-1 rounded-md bg-moss/20 px-2 py-1 text-[11px] font-medium text-moss transition-colors hover:bg-moss/30 disabled:opacity-40"
          >
            {pushing ? <Loader2 size={11} className="animate-spin" /> : <Upload size={11} />} commit
            and push
          </button>
        </div>
      </div>
    </Modal>
  );
}
