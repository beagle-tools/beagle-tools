// A GitHub review-comment thread rendered inline in the diff as a
// collapsed disclosure — including outdated/resolved threads (anchored
// via original_line). Local discussion attached to the thread renders
// BELOW the GitHub comments, and the composer can continue either.

import { Check, MessageSquare, Reply } from "lucide-react";
import { useState } from "react";
import type { ReactElement } from "react";

import { ipc } from "../lib/ipc";
import { useAppStore } from "../state/store";
import { pushGithubError, pushInfo } from "../state/toasts";
import type { GithubComment, LocalComment } from "../lib/types";
import { CommentThread, GithubCommentCard } from "./comments";
import { Button, GithubMark, Spinner } from "./ui";

interface Thread {
  root: LocalComment;
  replies: LocalComment[];
}

interface GithubThreadProps {
  root: GithubComment;
  replies: GithubComment[];
  localThreads: Thread[];
  outdated: boolean;
  /** GraphQL thread meta when known: resolution + node id. */
  resolved: boolean;
  threadId: string | null;
  onDiscuss: () => void;
}

export function GithubThread(props: GithubThreadProps): ReactElement {
  const { root, replies, localThreads } = props;
  const refreshBundle = useAppStore((s) => s.refreshBundle);
  // Unresolved threads start expanded; resolved ones start collapsed.
  const [open, setOpen] = useState(!props.resolved);
  const [resolving, setResolving] = useState(false);
  const count = 1 + replies.length;

  const resolve = () => {
    if (!props.threadId) return;
    setResolving(true);
    void ipc
      .resolveGithubThread(props.threadId)
      .then(async () => {
        pushInfo("thread resolved on github");
        await refreshBundle();
      })
      .catch((e: unknown) => {
        pushGithubError(e instanceof Error ? e.message : String(e));
      })
      .finally(() => {
        setResolving(false);
      });
  };
  // Bot comments often open with badge HTML (<a><img …>) — keep the
  // image alt text, drop every tag, then flatten the markdown.
  const preview = (() => {
    const text = root.body
      .replace(/<!--[\s\S]*?-->/g, "")
      .replace(/<img[^>]*alt="([^"]*)"[^>]*>/gi, " $1 ")
      .replace(/<[^>]+>/g, " ")
      .replace(/!?\[([^\]]*)\]\([^)]*\)/g, " $1 ")
      .replace(/[`#>*_|-]+/g, " ")
      .replace(/\s+/g, " ")
      .trim();
    return text.slice(0, 90) || "(no text — badges or images only)";
  })();

  return (
    <details
      data-nav-thread=""
      open={open}
      onToggle={(e) => {
        setOpen(e.currentTarget.open);
      }}
      className="border-y border-edge/60 border-l-4 border-l-fur/70 bg-fur/10"
    >
      <summary className="flex cursor-pointer items-center gap-2 px-4 py-1.5 text-[11px] text-muted hover:text-cream">
        <GithubMark size={10} />
        <span className="font-medium text-cream">{root.author.login}</span>
        <span className="inline-flex items-center gap-1">
          <MessageSquare size={10} /> {count}
        </span>
        {props.outdated ? (
          <span className="rounded-full bg-edge/60 px-1.5 py-0.5 text-[9px] uppercase">
            outdated
          </span>
        ) : null}
        {props.resolved ? (
          <span className="rounded-full bg-moss/15 px-1.5 py-0.5 text-[9px] uppercase text-moss">
            resolved
          </span>
        ) : null}
        {localThreads.length > 0 ? (
          <span className="rounded-full bg-sky/15 px-1.5 py-0.5 text-[9px] uppercase text-sky">
            {localThreads.length} local
          </span>
        ) : null}
        <span className="min-w-0 flex-1 truncate">{preview}</span>
      </summary>
      <div className="space-y-2 px-4 pb-2.5">
        <GithubCommentCard comment={root} />
        {replies.map((reply) => (
          <div key={reply.id} className="ml-3 border-l-2 border-edge/70 pl-3">
            <GithubCommentCard comment={reply} />
          </div>
        ))}
        {localThreads.map((thread) => (
          <div key={thread.root.id} className="rounded-lg border border-edge bg-panel-2/40 p-2.5">
            <CommentThread root={thread.root} replies={thread.replies} />
          </div>
        ))}
        <div className="flex items-center gap-1.5">
          <Button onClick={props.onDiscuss} title="reply locally or straight on github">
            <Reply size={11} /> discuss
          </Button>
          {!props.resolved && props.threadId !== null ? (
            resolving ? (
              <Spinner label="resolving…" />
            ) : (
              <Button onClick={resolve} title="mark this thread resolved on GitHub">
                <Check size={11} /> resolve
              </Button>
            )
          ) : null}
        </div>
      </div>
    </details>
  );
}
