// The agent's PR-level write-up, shown above the diff next to the PR's
// own description. One per PR: a re-review hands the agent this text to
// revise, so it's the running account rather than a per-run log. You
// can reply underneath it, which turns the summary into a thread the
// agent answers in — the same threading the inline comments use, rooted
// on a PR-level comment seeded with the summary.

import { Bot, CornerDownLeft, Loader2, Maximize2, Trash2, User } from "lucide-react";
import { useRef, useState } from "react";
import type { ReactElement } from "react";
import { create } from "zustand";

import { relativeTime } from "../lib/format";
import { useStickToBottom } from "../lib/stickToBottom";
import { ipc } from "../lib/ipc";
import type { LocalComment, PrSummary, RepoRef } from "../lib/types";
import { loadAgent, saveAgent } from "../state/persist";
import { isWorking } from "../state/runBoard";
import { useAppStore } from "../state/store";
import { pushInfo } from "../state/toasts";
import { MarkdownBody } from "./Markdown";
import { Modal, Pill, Spinner } from "./ui";

export function SummarySection(): ReactElement | null {
  const summary = useAppStore((s) => s.bundle?.summary ?? null);
  const headSha = useAppStore((s) => s.bundle?.detail.pull_request.head_sha);
  const comments = useAppStore((s) => s.bundle?.comments);
  const deleteSummary = useAppStore((s) => s.deleteSummary);
  if (summary === null) return null;

  // Written against commits that have since been pushed over — say so
  // rather than quietly presenting it as current.
  const stale = headSha !== undefined && headSha !== summary.head_sha;
  const root = summary.thread_root_id;
  // Oldest first — it reads as a conversation, not a feed.
  const replies =
    root === null
      ? []
      : (comments ?? [])
          .filter((c) => c.parent_id === root)
          .slice()
          .sort((a, b) => a.created_at.localeCompare(b.created_at));

  return (
    <details open className="mt-2">
      <summary className="flex cursor-pointer items-center gap-1.5 text-[11px] text-muted hover:text-cream">
        <Bot size={10} /> agent summary
        <span className="text-muted/70">
          · {summary.agent_name} · {relativeTime(summary.updated_at)}
        </span>
        {stale ? <Pill tone="amber">new commits since</Pill> : null}
        <button
          type="button"
          title="open the summary in a floating pane"
          onClick={(e) => {
            e.preventDefault();
            useSummaryPane.getState().setOpen(true);
          }}
          className="ml-auto inline-flex size-5 items-center justify-center rounded text-muted transition-colors hover:bg-panel-2 hover:text-cream"
        >
          <Maximize2 size={11} />
        </button>
        <button
          type="button"
          title="delete this summary — the next review writes a fresh one"
          onClick={(e) => {
            e.preventDefault();
            void deleteSummary();
          }}
          className="inline-flex size-5 items-center justify-center rounded text-muted transition-colors hover:bg-ember/15 hover:text-ember"
        >
          <Trash2 size={11} />
        </button>
      </summary>
      <div className="mt-2 max-h-96 overflow-y-auto rounded-lg border border-moss/30 bg-moss/5 p-3">
        <SummaryBody summary={summary} replies={replies} />
      </div>
    </details>
  );
}

/** The summary and its discussion, shared by the inline section and the
 * floating pane so they can't drift apart. */
function SummaryBody({ summary, replies }: { summary: PrSummary; replies: LocalComment[] }) {
  // Same signal the inline threads give: the run is spawned, the reply
  // lands minutes later, and silence in between reads as failure.
  const answering = useAppStore((s) =>
    s.runs.find(
      (r) =>
        r.purpose === "thread reply" &&
        r.target_comment_id === summary.thread_root_id &&
        isWorking(r),
    ),
  )?.agent_name;

  // Follow the conversation down as it grows — unless you've scrolled
  // up to re-read something, in which case stay put.
  const endRef = useRef<HTMLDivElement | null>(null);
  useStickToBottom(endRef, `${String(replies.length)}:${answering ?? ""}`);

  return (
    <>
      <MarkdownBody text={summary.body} />
      {replies.length > 0 ? <Thread replies={replies} /> : null}
      {answering === undefined ? null : (
        <div className="mt-3 border-t border-moss/20 pt-3">
          <Spinner label={`${answering} is answering…`} />
        </div>
      )}
      <ReplyBox repo={summary.repo} number={summary.pr_number} agent={summary.agent_name} />
      <div ref={endRef} />
    </>
  );
}

/** The same summary, floated over the diff — room to read a long one
 * without the diff squeezing it into a scroll box. */
export function SummaryPane(): ReactElement | null {
  const summary = useAppStore((s) => s.bundle?.summary ?? null);
  const comments = useAppStore((s) => s.bundle?.comments);
  const open = useSummaryPane((s) => s.open);
  const setOpen = useSummaryPane((s) => s.setOpen);
  if (!open || summary === null) return null;
  const root = summary.thread_root_id;
  const replies =
    root === null
      ? []
      : (comments ?? [])
          .filter((c) => c.parent_id === root)
          .slice()
          .sort((a, b) => a.created_at.localeCompare(b.created_at));
  return (
    <Modal
      title={
        <span className="flex items-center gap-2">
          <Bot size={12} /> agent summary
          <span className="text-muted">
            {summary.agent_name} · {relativeTime(summary.updated_at)}
          </span>
        </span>
      }
      onClose={() => {
        setOpen(false);
      }}
    >
      <SummaryBody summary={summary} replies={replies} />
    </Modal>
  );
}

interface SummaryPaneStore {
  open: boolean;
  setOpen: (open: boolean) => void;
}

export const useSummaryPane = create<SummaryPaneStore>((set) => ({
  open: false,
  setOpen: (open) => {
    set({ open });
  },
}));

function Thread({ replies }: { replies: LocalComment[] }) {
  return (
    <div className="mt-3 space-y-2 border-t border-moss/20 pt-3">
      {replies.map((reply) => {
        // Who said what has to be readable at a glance: your messages
        // sit in sky, the agent's in the same moss as the summary it's
        // talking about.
        const mine = reply.author_kind !== "agent";
        return (
          <div
            key={reply.id}
            className={`rounded-lg border-l-2 p-2 ${
              mine ? "border-sky/70 bg-sky/10" : "border-moss/60 bg-panel-2/70"
            }`}
          >
            <div className="mb-1 flex items-center gap-1.5 text-[10px] text-muted">
              {mine ? (
                <User size={10} className="text-sky" />
              ) : (
                <Bot size={10} className="text-moss" />
              )}
              <span className={mine ? "font-medium text-sky" : "font-medium text-moss"}>
                {reply.author_name}
              </span>
              <span>· {relativeTime(reply.created_at)}</span>
            </div>
            <MarkdownBody text={reply.body} />
          </div>
        );
      })}
    </div>
  );
}

/** Ask the agent something about its own summary. Sending starts a run
 * that answers into the thread, so the button stays disabled until the
 * call returns rather than letting a double-tap start two. */
function ReplyBox({ repo, number, agent }: { repo: RepoRef; number: number; agent: string }) {
  const specs = useAppStore((s) => s.agentSpecs);
  const [body, setBody] = useState("");
  const [sending, setSending] = useState(false);
  // Default to whoever wrote the summary — you're talking to them.
  const [who, setWho] = useState(() => (specs.some((s) => s.name === agent) ? agent : loadAgent()));
  const slug = `${repo.owner}/${repo.name}`;

  const send = () => {
    if (!body.trim() || sending || !who) return;
    setSending(true);
    ipc
      .replyToSummary(who, slug, number, body.trim())
      .then(() => {
        setBody("");
        pushInfo(`${who} is answering…`);
      })
      .catch((e: unknown) => {
        console.error("summary reply failed", e);
        pushInfo("couldn't send that reply");
      })
      .finally(() => {
        setSending(false);
      });
  };

  if (specs.length === 0) return null;

  return (
    <div className="mt-3 flex items-end gap-1.5 border-t border-moss/20 pt-3">
      <textarea
        value={body}
        onChange={(e) => {
          setBody(e.target.value);
        }}
        onKeyDown={(e) => {
          if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) send();
        }}
        placeholder={`ask ${who || "the agent"} about this summary — ⌘↵ to send`}
        className="min-h-9 flex-1 resize-y rounded-lg border border-edge bg-ground p-2 text-xs leading-relaxed text-cream outline-none focus:border-sky"
      />
      <select
        value={who}
        onChange={(e) => {
          setWho(e.target.value);
          saveAgent(e.target.value);
        }}
        title="agent to answer"
        className="h-8 max-w-28 cursor-pointer rounded-lg border border-edge bg-panel-2 px-1.5 text-[11px] text-muted"
      >
        {specs.map((spec) => (
          <option key={spec.name} value={spec.name}>
            {spec.name}
          </option>
        ))}
      </select>
      <button
        type="button"
        onClick={send}
        disabled={!body.trim() || sending}
        title="send (⌘↵)"
        className="inline-flex h-8 items-center gap-1 rounded-lg bg-sky-deep px-2 text-[11px] font-medium text-white transition-colors hover:bg-sky disabled:pointer-events-none disabled:opacity-40"
      >
        {sending ? <Loader2 size={11} className="animate-spin" /> : <CornerDownLeft size={11} />}
      </button>
    </div>
  );
}
