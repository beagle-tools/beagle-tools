// Compact summary cards for the right-hand comments panel: local
// threads, GitHub comments, and GitHub review bodies. Clicking a card
// opens the floating pane (selection managed by CommentsPanel).

import type { ReactElement } from "react";
import { AtSign, Bot, MessageSquare, Send, User } from "lucide-react";

import { mentionsUser, relativeTime } from "../lib/format";
import type { GithubComment, GithubReview, LocalComment } from "../lib/types";
import { useAppStore } from "../state/store";
import { MarkdownBody } from "./Markdown";
import { GithubMark, Pill, severityTone } from "./ui";

export interface Thread {
  root: LocalComment;
  replies: LocalComment[];
}

export type Selected =
  | { kind: "local"; id: string }
  | { kind: "github"; id: number }
  | { kind: "review"; id: number }
  | { kind: "new-general" }
  | null;

export function ThreadSummary(props: {
  thread: Thread;
  onOpen: (s: Selected) => void;
}): ReactElement {
  const { root, replies } = props.thread;
  const viewer = useAppStore((s) => s.viewer);
  const mentioned = [root, ...replies].some((c) => mentionsUser(c.body, viewer));
  return (
    <button
      type="button"
      onClick={() => {
        props.onOpen({ kind: "local", id: root.id });
      }}
      className="animate-fade-up block w-full rounded-lg border border-edge bg-panel p-2.5 text-left transition-all hover:border-sky/40 hover:bg-panel-2/60"
    >
      <div className="mb-1 flex flex-wrap items-center gap-1.5 text-[11px]">
        <span className="inline-flex items-center gap-1 font-medium text-cream">
          {root.author_kind === "agent" ? <Bot size={11} /> : <User size={11} />}
          {root.author_name}
        </span>
        <Pill tone={severityTone(root.severity)}>{root.severity}</Pill>
        {root.status !== "open" ? <Pill tone="muted">{root.status}</Pill> : null}
        {root.posted_github_id !== null ? (
          <Pill tone="moss">
            <Send size={9} /> posted
          </Pill>
        ) : null}
        {mentioned ? (
          <Pill tone="amber">
            <AtSign size={9} /> you
          </Pill>
        ) : null}
        <span className="ml-auto text-[10px] text-muted">{relativeTime(root.created_at)}</span>
      </div>
      <div className="truncate font-mono text-[10px] text-sky">
        {root.path ? `${root.path}:${String(root.line)}` : "PR comment"}
      </div>
      <div className="md-clamp mt-0.5">
        <MarkdownBody text={root.body} />
      </div>
      {replies.length > 0 ? (
        <div className="mt-1 inline-flex items-center gap-1 text-[10px] text-muted">
          <MessageSquare size={10} /> {replies.length} {replies.length === 1 ? "reply" : "replies"}
        </div>
      ) : null}
    </button>
  );
}

export function GithubSummary(props: {
  comment: GithubComment;
  onOpen: (s: Selected) => void;
}): ReactElement {
  const { comment } = props;
  const viewer = useAppStore((s) => s.viewer);
  return (
    <button
      type="button"
      onClick={() => {
        props.onOpen({ kind: "github", id: comment.id });
      }}
      className="animate-fade-up block w-full rounded-lg border border-edge border-l-4 border-l-fur/70 bg-fur/10 p-2.5 text-left transition-all hover:border-sky/40"
    >
      <div className="mb-1 flex items-center gap-1.5 text-[11px]">
        <span className="inline-flex items-center gap-1 font-medium text-cream">
          <User size={11} /> {comment.author.login}
        </span>
        <Pill tone="github">
          <GithubMark size={9} /> github
        </Pill>
        {mentionsUser(comment.body, viewer) ? (
          <Pill tone="amber">
            <AtSign size={9} /> you
          </Pill>
        ) : null}
        <span className="ml-auto text-[10px] text-muted">{relativeTime(comment.created_at)}</span>
      </div>
      {comment.path ? (
        <div className="truncate font-mono text-[10px] text-muted">
          {comment.path}
          {comment.line !== null ? `:${String(comment.line)}` : ""}
        </div>
      ) : null}
      <div className="md-clamp mt-0.5">
        <MarkdownBody text={comment.body} />
      </div>
    </button>
  );
}

export function verdictTone(verdict: GithubReview["verdict"]): "moss" | "ember" | "muted" {
  switch (verdict) {
    case "approved":
      return "moss";
    case "changes_requested":
      return "ember";
    case "commented":
    case "dismissed":
    case "pending":
      return "muted";
  }
}

/** A GitHub review event that carried body text (mentions often live
 * here — they're invisible on the checks pills otherwise). */
export function ReviewSummary(props: {
  review: GithubReview;
  onOpen: (s: Selected) => void;
}): ReactElement {
  const { review } = props;
  const viewer = useAppStore((s) => s.viewer);
  return (
    <button
      type="button"
      onClick={() => {
        props.onOpen({ kind: "review", id: review.id });
      }}
      className="animate-fade-up block w-full rounded-lg border border-edge border-l-4 border-l-fur/70 bg-fur/10 p-2.5 text-left transition-all hover:border-sky/40"
    >
      <div className="mb-1 flex items-center gap-1.5 text-[11px]">
        <span className="inline-flex items-center gap-1 font-medium text-cream">
          <User size={11} /> {review.author.login}
        </span>
        <Pill tone="github">
          <GithubMark size={9} /> review
        </Pill>
        <Pill tone={verdictTone(review.verdict)}>{review.verdict.replace("_", " ")}</Pill>
        {mentionsUser(review.body, viewer) ? (
          <Pill tone="amber">
            <AtSign size={9} /> you
          </Pill>
        ) : null}
        <span className="ml-auto text-[10px] text-muted">
          {review.submitted_at ? relativeTime(review.submitted_at) : ""}
        </span>
      </div>
      <div className="md-clamp mt-0.5">
        <MarkdownBody text={review.body} />
      </div>
    </button>
  );
}
