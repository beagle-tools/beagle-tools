// The composer under a comment: draft locally (default) or reply
// straight on GitHub. Shared by the comment list's floating panes.

import { useState } from "react";
import type { ReactElement } from "react";

import { GithubReplyComposer } from "./GithubReplyComposer";
import { InlineCommentForm } from "./InlineCommentForm";

/** Composer area with a mode toggle: draft a local thread (default) or
 * reply straight on GitHub. */
export function ReplySection(props: {
  path: string;
  line: number;
  githubCommentId?: number | undefined;
  reviewCommentId?: number | undefined;
  onDone?: (() => void) | undefined;
}): ReactElement {
  const [mode, setMode] = useState<"local" | "github">("local");
  return (
    <div className="mt-4 border-t border-edge pt-3">
      <div className="mb-2 flex items-center gap-1.5">
        {(["local", "github"] as const).map((m) => (
          <button
            key={m}
            type="button"
            onClick={() => {
              setMode(m);
            }}
            className={`rounded-full px-2.5 py-1 text-[10px] font-medium transition-colors ${
              mode === m ? "bg-sky/20 text-sky" : "bg-panel-2 text-muted hover:text-cream"
            }`}
          >
            {m === "local" ? "reply locally" : "reply on github"}
          </button>
        ))}
        <span className="ml-auto text-[10px] text-muted">
          {mode === "local"
            ? "stays in Tandem until you post it"
            : "posts under your account after confirm"}
        </span>
      </div>
      {mode === "local" ? (
        <FreshComposer
          path={props.path}
          line={props.line}
          githubCommentId={props.githubCommentId}
          onDone={props.onDone}
        />
      ) : (
        <GithubReplyComposer reviewCommentId={props.reviewCommentId} onDone={props.onDone} />
      )}
    </div>
  );
}

/** Composer that resets itself after each send so it can live inside a
 * modal permanently. */
function FreshComposer(props: {
  path: string;
  line: number;
  githubCommentId?: number | undefined;
  onDone?: (() => void) | undefined;
}) {
  const [generation, setGeneration] = useState(0);
  return (
    <InlineCommentForm
      key={generation}
      path={props.path}
      line={props.line}
      side="new"
      githubCommentId={props.githubCommentId}
      onDone={() => {
        setGeneration((g) => g + 1);
        props.onDone?.();
      }}
    />
  );
}
