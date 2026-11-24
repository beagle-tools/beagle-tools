// Merge controls, shown only on the viewer's own PRs: merge now (with
// method), update-from-base, and auto-merge (which on merge-queue
// repos adds the PR to the queue).

import { GitMerge, RefreshCw, Timer } from "lucide-react";
import { useEffect, useState } from "react";
import type { ReactElement } from "react";

import { ipc } from "../lib/ipc";
import { isReadyToMerge } from "../lib/ready";
import type { MergeOptions } from "../lib/types";
import type { PullRequest } from "../lib/types";
import { useAppStore } from "../state/store";
import { pushGithubError, pushInfo } from "../state/toasts";
import { Button, Spinner } from "./ui";

const optionsCache = new Map<string, MergeOptions>();

/** One-line verdict (and its color class) for the PR's merge state. */
function readinessOf(ready: boolean, state: string): { label: string; cls: string } {
  return ready
    ? { label: "approved and green — ready to merge", cls: "text-moss" }
    : state === "clean"
      ? { label: "ready to merge", cls: "text-moss" }
      : state === "blocked"
        ? { label: "blocked — approvals or checks still required", cls: "text-ember" }
        : state === "dirty"
          ? { label: "merge conflicts with base", cls: "text-ember" }
          : state === "behind"
            ? { label: "behind base — update first", cls: "text-amber" }
            : state === "unstable"
              ? { label: "checks still running", cls: "text-amber" }
              : state === "draft"
                ? { label: "draft PR — mark ready on GitHub first", cls: "text-muted" }
                : { label: "merge state unknown — GitHub decides on submit", cls: "text-muted" };
}

export function MergeControls({ pr }: { pr: PullRequest }): ReactElement | null {
  const viewer = useAppStore((s) => s.viewer);
  const collaborators = useAppStore((s) => s.collaborators);
  const refreshBundle = useAppStore((s) => s.refreshBundle);
  const refreshPrs = useAppStore((s) => s.refreshPrs);
  const [open, setOpen] = useState(false);
  const [method, setMethod] = useState("squash");
  const repoSlug = `${pr.repo.owner}/${pr.repo.name}`;
  const [opts, setOpts] = useState<MergeOptions | null>(optionsCache.get(repoSlug) ?? null);

  useEffect(() => {
    if (!open || opts !== null) return;
    ipc
      .repoMergeOptions(repoSlug)
      .then((fetched) => {
        optionsCache.set(repoSlug, fetched);
        setOpts(fetched);
      })
      .catch(console.warn);
  }, [open, opts, repoSlug]);

  // Keep the selected method inside what the repo actually allows.
  const allowed = [
    opts?.squash !== false ? "squash" : null,
    opts?.merge !== false ? "merge" : null,
    opts?.rebase !== false ? "rebase" : null,
  ].filter((m): m is string => m !== null);
  useEffect(() => {
    if (allowed.length > 0 && !allowed.includes(method)) {
      setMethod(allowed[0] ?? "squash");
    }
  }, [allowed, method]);
  const [confirming, setConfirming] = useState(false);
  const [working, setWorking] = useState<string | null>(null);

  // Match GitHub: the author or anyone with push access can merge.
  const canMerge =
    viewer !== null && (viewer === pr.author.login || collaborators.includes(viewer));
  if (!canMerge) return null;
  const repo = `${pr.repo.owner}/${pr.repo.name}`;
  const state = pr.mergeable_state ?? "";
  const ready = isReadyToMerge(pr);
  const mergeableNow = ["clean", "has_hooks", "unstable", "behind", ""].includes(state);
  const readiness = readinessOf(ready, state);

  const run = (label: string, action: () => Promise<null>) => {
    setWorking(label);
    action()
      .then(async () => {
        setConfirming(false);
        setOpen(false);
        pushInfo(`${label} — done`);
        await refreshBundle();
        await refreshPrs();
      })
      .catch((e: unknown) => {
        pushGithubError(e instanceof Error ? e.message : String(e));
      })
      .finally(() => {
        setWorking(null);
      });
  };

  return (
    <span className="relative">
      <Button
        kind={ready ? "primary" : "ghost"}
        onClick={() => {
          setOpen((o) => !o);
        }}
        title={
          ready
            ? "ready — merge controls"
            : `not ready yet (${state || "state unknown"}) — auto-merge available inside`
        }
      >
        <GitMerge size={12} /> merge…
      </Button>

      {open ? (
        <div className="animate-fade-up absolute right-0 top-full z-40 mt-2 w-80 rounded-xl border border-edge bg-panel p-3 shadow-2xl">
          <div className="mb-1 text-[11px] font-medium uppercase tracking-wide text-muted">
            merge controls
          </div>
          <div className={`mb-2 text-[11px] ${readiness.cls}`}>{readiness.label}</div>
          <label className="mb-2 flex items-center gap-2 text-xs text-muted">
            method
            <select
              value={method}
              onChange={(e) => {
                setMethod(e.target.value);
              }}
              className="flex-1 rounded-md border border-edge bg-ground px-2 py-1 text-xs text-cream"
            >
              {allowed.includes("squash") ? <option value="squash">squash</option> : null}
              {allowed.includes("merge") ? <option value="merge">merge commit</option> : null}
              {allowed.includes("rebase") ? <option value="rebase">rebase</option> : null}
            </select>
          </label>

          <MergeActions
            working={working}
            opts={opts}
            mergeableNow={mergeableNow}
            confirming={confirming}
            setConfirming={setConfirming}
            run={run}
            repo={repo}
            number={pr.number}
            method={method}
          />
        </div>
      ) : null}
    </span>
  );
}

/** Update-from-base, auto-merge, and the two-step "merge now" — or a
 * spinner while one of them runs. */
function MergeActions({
  working,
  opts,
  mergeableNow,
  confirming,
  setConfirming,
  run,
  repo,
  number,
  method,
}: {
  working: string | null;
  opts: MergeOptions | null;
  mergeableNow: boolean;
  confirming: boolean;
  setConfirming: (confirming: boolean) => void;
  run: (label: string, action: () => Promise<null>) => void;
  repo: string;
  number: number;
  method: string;
}): ReactElement {
  if (working) return <Spinner label={`${working}…`} />;
  return (
    <div className="flex flex-col gap-1.5">
      <Button
        onClick={() => {
          run("updating branch", () => ipc.updatePrBranch(repo, number));
        }}
        title="merge the base branch into this PR branch"
      >
        <RefreshCw size={11} /> update from base
      </Button>
      <Button
        disabled={opts !== null && !opts.auto_merge}
        onClick={() => {
          run("enabling auto-merge", () => ipc.enableAutoMerge(repo, number, method));
        }}
        title={
          opts !== null && !opts.auto_merge
            ? "auto-merge is not enabled on this repository (a GitHub repo setting)"
            : "merge automatically once requirements pass (joins the merge queue on queue repos)"
        }
      >
        <Timer size={11} /> auto-merge when ready
        {opts !== null && !opts.auto_merge ? " (unavailable)" : ""}
      </Button>
      {!mergeableNow ? (
        <Button
          disabled
          onClick={() => {
            /* gated by merge state */
          }}
          title="GitHub reports this PR can't merge right now — use auto-merge instead"
        >
          <GitMerge size={11} /> merge now
        </Button>
      ) : confirming ? (
        <div className="flex gap-1.5">
          <Button
            kind="danger"
            title="this WILL merge the PR now"
            onClick={() => {
              run("merging", () => ipc.mergePr(repo, number, method));
            }}
          >
            <GitMerge size={11} /> confirm merge
          </Button>
          <Button
            onClick={() => {
              setConfirming(false);
            }}
          >
            cancel
          </Button>
        </div>
      ) : (
        <Button
          kind="primary"
          onClick={() => {
            setConfirming(true);
          }}
        >
          <GitMerge size={11} /> merge now
        </Button>
      )}
    </div>
  );
}
