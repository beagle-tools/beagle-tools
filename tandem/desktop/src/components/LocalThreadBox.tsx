// Standalone local thread in the diff with a hide toggle whose state is
// remembered per comment (localStorage).

import { Eye, EyeOff } from "lucide-react";
import { useState } from "react";
import type { ReactElement } from "react";

import type { LocalComment } from "../lib/types";
import { CommentThread } from "./comments";

interface Thread {
  root: LocalComment;
  replies: LocalComment[];
}

const hideKey = (id: string) => `tandem-hide-${id}`;

export function LocalThreadBox({ thread }: { thread: Thread }): ReactElement {
  const [hidden, setHidden] = useState(
    () => localStorage.getItem(hideKey(thread.root.id)) === "true",
  );
  const toggle = () => {
    const next = !hidden;
    setHidden(next);
    localStorage.setItem(hideKey(thread.root.id), String(next));
  };

  if (hidden) {
    return (
      <button
        type="button"
        onClick={toggle}
        title="show this local thread"
        className="flex w-full items-center gap-2 border-y border-edge/40 bg-panel-2/30 px-4 py-1 text-left text-[11px] text-muted transition-colors hover:text-cream"
      >
        <Eye size={11} />
        <span className="font-medium">{thread.root.author_name}</span>
        <span className="min-w-0 flex-1 truncate">
          {thread.root.body.replace(/\s+/g, " ").slice(0, 80)}
        </span>
        <span className="shrink-0 rounded-full bg-edge/60 px-1.5 py-0.5 text-[9px] uppercase">
          hidden · local
        </span>
      </button>
    );
  }

  return (
    <div
      data-nav-thread=""
      className="relative border-y border-edge/60 bg-panel-2/70 py-2 pl-4 pr-11"
    >
      <button
        type="button"
        onClick={toggle}
        title="hide this local thread (remembered)"
        className="absolute right-2 top-2 inline-flex size-6 items-center justify-center rounded-md text-muted transition-colors hover:bg-panel-2 hover:text-cream"
      >
        <EyeOff size={12} />
      </button>
      <CommentThread root={thread.root} replies={thread.replies} />
    </div>
  );
}
