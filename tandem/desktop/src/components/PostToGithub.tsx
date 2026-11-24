// The one button in a comment card that leaves the machine. Split from
// comments.tsx for the 400-line rule; the confirm-first shape is the
// point, so it keeps its own file rather than being inlined anywhere.

import { ExternalLink, Send } from "lucide-react";
import { useState } from "react";
import type { ReactElement } from "react";

import { openExternal, postedCommentUrl } from "../lib/open";
import type { LocalComment } from "../lib/types";
import { useAppStore } from "../state/store";
import { Button, Pill, Spinner } from "./ui";

/** Explicit, two-step "post this comment body to GitHub" control. */
export function PostToGithub({ comment }: { comment: LocalComment }): ReactElement {
  const postToGithub = useAppStore((s) => s.postToGithub);
  const [confirming, setConfirming] = useState(false);
  const [posting, setPosting] = useState(false);

  const postedUrl = postedCommentUrl(comment);
  if (postedUrl !== null) {
    return (
      <button
        type="button"
        title="posted — open it on github.com"
        onClick={() => {
          openExternal(postedUrl);
        }}
        className="inline-flex"
      >
        <Pill tone="moss">
          <Send size={10} /> posted <ExternalLink size={9} />
        </Pill>
      </button>
    );
  }
  if (posting) return <Spinner label="posting…" />;
  if (confirming) {
    return (
      <>
        <Button
          kind="danger"
          onClick={() => {
            setPosting(true);
            void postToGithub(comment.id).then(() => {
              setPosting(false);
              setConfirming(false);
            });
          }}
          title="this WILL post to GitHub"
        >
          <Send size={11} /> confirm post
        </Button>
        <Button
          onClick={() => {
            setConfirming(false);
          }}
        >
          cancel
        </Button>
      </>
    );
  }
  return (
    <Button
      onClick={() => {
        setConfirming(true);
      }}
      title="post this comment to GitHub (asks to confirm)"
    >
      <Send size={11} /> post to github
    </Button>
  );
}
