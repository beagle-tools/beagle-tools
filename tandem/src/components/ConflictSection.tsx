// Conflict resolution lives above the comments, not inside them. A
// briefing buried in the general-comment list is a thing you have to
// find; this is a thing you act on: collapsed to one line, opening the
// findings in a floating pane, with the two buttons that matter.

import { AlertTriangle, ChevronRight, GitMerge, Loader2, Upload } from "lucide-react";
import { useEffect, useState } from "react";
import type { ReactElement } from "react";

import { relativeTime } from "../lib/format";
import { ipc } from "../lib/ipc";
import type { AgentRun, AgentSpec, LocalComment, ResolutionState } from "../lib/types";
import { loadAgent, saveAgent } from "../state/persist";
import { isWorking } from "../state/runBoard";
import { useAppStore } from "../state/store";
import { pushGithubError, pushInfo } from "../state/toasts";
import { CommentThread } from "./comments";
import { Modal, Pill, Spinner } from "./ui";

/** Conflict work for this PR: the agent's briefings and resolutions,
 * which carry the "conflict" tag, plus their replies. */
export function conflictThreads(comments: LocalComment[]): LocalComment[] {
  return comments.filter((c) => c.tag === "conflict" && c.parent_id === null);
}

export function ConflictSection(): ReactElement | null {
  const bundle = useAppStore((s) => s.bundle);
  const specs = useAppStore((s) => s.agentSpecs);
  const runs = useAppStore((s) => s.runs);
  const [open, setOpen] = useState(false);
  const [starting, setStarting] = useState(false);
  const [agentName, setAgentName] = useState(loadAgent);
  const [pushing, setPushing] = useState(false);
  const [state, setState] = useState<ResolutionState | null>(null);

  const pr = bundle?.detail.pull_request;
  const comments = bundle?.comments ?? [];
  const roots = conflictThreads(comments);
  const latest = roots.reduce<LocalComment | null>(
    (best, c) => (best === null || c.created_at > best.created_at ? c : best),
    null,
  );
  const running = runs.find((r) => r.purpose.startsWith("conflict") && isWorking(r));
  const conflicted = pr?.mergeable_state === "dirty";

  // How the checkout stands — refreshed whenever a run finishes, since
  // that's when markers disappear.
  useEffect(() => {
    if (pr === undefined || (!conflicted && latest === null)) return;
    ipc
      .conflictResolutionState(`${pr.repo.owner}/${pr.repo.name}`, pr.number)
      .then(setState)
      .catch(() => {
        setState(null);
      });
  }, [pr, conflicted, latest, running]);

  if (pr === undefined || (!conflicted && latest === null)) return null;
  const repo = `${pr.repo.owner}/${pr.repo.name}`;
  const agent = specs.some((s) => s.name === agentName) ? agentName : (specs[0]?.name ?? "");

  const resolve = () => {
    if (!agent) {
      pushGithubError("no agents configured — add one in settings");
      return;
    }
    setStarting(true);
    ipc
      .startConflictFix(agent, repo, pr.number)
      .then(() => {
        pushInfo(`${agent} is resolving the conflicts`);
      })
      .catch((e: unknown) => {
        pushGithubError(e instanceof Error ? e.message : String(e));
      })
      .finally(() => {
        setStarting(false);
      });
  };

  const push = () => {
    setPushing(true);
    ipc
      .pushConflictResolution(repo, pr.number)
      .then((sha) => {
        pushInfo(`pushed the resolution (${sha.slice(0, 7)})`);
      })
      .catch((e: unknown) => {
        pushGithubError(e instanceof Error ? e.message : String(e));
      })
      .finally(() => {
        setPushing(false);
      });
  };

  return (
    <div className="mb-1.5 overflow-hidden rounded-lg border border-ember/40 bg-ember/5">
      <div className="flex items-center gap-1.5 px-2.5 py-1.5 text-[11px] font-medium uppercase tracking-wide text-ember">
        <GitMerge size={11} /> conflict resolution
        {state?.ready === true ? <Pill tone="moss">resolved</Pill> : null}
      </div>

      {latest === null ? (
        <div className="px-2.5 pb-2 text-[11px] leading-relaxed text-muted">
          this branch conflicts with {pr.base_ref}. Tandem fetches the base and starts the merge,
          then the agent resolves what it can judge and asks about the rest.
        </div>
      ) : (
        <button
          type="button"
          onClick={() => {
            setOpen(true);
          }}
          className="flex w-full items-center gap-1.5 px-2.5 pb-2 text-left text-[11px] text-muted transition-colors hover:text-cream"
        >
          <ChevronRight size={11} className="shrink-0" />
          <span className="truncate">
            findings · {latest.author_name} · {relativeTime(latest.created_at)}
          </span>
        </button>
      )}

      {state !== null && state.unresolved.length > 0 ? (
        <div className="flex items-start gap-1.5 border-t border-ember/30 px-2.5 py-1.5 text-[10px] leading-relaxed text-amber">
          <AlertTriangle size={11} className="mt-px shrink-0" />
          <span>
            left for you: <span className="font-mono">{state.unresolved.join(", ")}</span> — answer
            in the findings thread, then resolve again
          </span>
        </div>
      ) : null}

      <ConflictActions
        running={running}
        starting={starting}
        agent={agent}
        specs={specs}
        setAgentName={setAgentName}
        onResolve={resolve}
        ready={state?.ready === true}
        pushing={pushing}
        onPush={push}
      />

      {open && latest !== null ? (
        <FindingsModal
          latest={latest}
          comments={comments}
          onClose={() => {
            setOpen(false);
          }}
        />
      ) : null}
    </div>
  );
}

/** The bottom bar: agent picker + "resolve", or a spinner while a run
 * works, plus "push" once the checkout has no conflict markers left. */
function ConflictActions({
  running,
  starting,
  agent,
  specs,
  setAgentName,
  onResolve,
  ready,
  pushing,
  onPush,
}: {
  running: AgentRun | undefined;
  starting: boolean;
  agent: string;
  specs: AgentSpec[];
  setAgentName: (name: string) => void;
  onResolve: () => void;
  ready: boolean;
  pushing: boolean;
  onPush: () => void;
}): ReactElement {
  return (
    <div className="flex items-center gap-1.5 border-t border-ember/30 bg-ember/5 px-2.5 py-1.5">
      {running !== undefined || starting ? (
        <Spinner
          label={`${running?.agent_name ?? agent} is ${
            running?.purpose === "conflict analysis" ? "analyzing" : "resolving"
          }…`}
        />
      ) : (
        <>
          <select
            value={agent}
            onChange={(e) => {
              setAgentName(e.target.value);
              saveAgent(e.target.value);
            }}
            title="agent to work the conflict"
            className="h-6 max-w-28 cursor-pointer rounded-md border border-edge bg-panel-2 px-1 text-[10px] text-muted"
          >
            {specs.map((spec) => (
              <option key={spec.name} value={spec.name}>
                {spec.name}
              </option>
            ))}
          </select>
          <button
            type="button"
            onClick={onResolve}
            title="fetch the base, start the merge, and have the agent resolve it"
            className="inline-flex items-center gap-1 rounded-md bg-ember/20 px-2 py-1 text-[11px] font-medium text-ember transition-colors hover:bg-ember/30"
          >
            <GitMerge size={11} /> resolve the conflicts
          </button>
        </>
      )}
      {ready ? (
        <button
          type="button"
          onClick={onPush}
          disabled={pushing}
          title="commit the merge and push it to the PR branch"
          className="inline-flex items-center gap-1 rounded-md bg-moss/20 px-2 py-1 text-[11px] font-medium text-moss transition-colors hover:bg-moss/30 disabled:opacity-40"
        >
          {pushing ? <Loader2 size={11} className="animate-spin" /> : <Upload size={11} />} push the
          resolution
        </button>
      ) : null}
    </div>
  );
}

/** The latest findings thread in a floating pane. */
function FindingsModal({
  latest,
  comments,
  onClose,
}: {
  latest: LocalComment;
  comments: LocalComment[];
  onClose: () => void;
}): ReactElement {
  return (
    <Modal
      title={
        <span className="inline-flex items-center gap-1.5">
          <GitMerge size={12} /> conflict findings
        </span>
      }
      onClose={onClose}
    >
      <CommentThread root={latest} replies={comments.filter((c) => c.parent_id === latest.id)} />
    </Modal>
  );
}
