// Inline comment composer for the differ: anchors to a line (or a
// range), supports @agent mentions, and can attach the new local thread
// to an existing GitHub comment.

import { useState } from "react";
import type { ReactElement } from "react";

import type { CommentSeverity, DiffSide } from "../lib/types";
import { useAppStore } from "../state/store";
import { GithubReplyComposer } from "./GithubReplyComposer";
import { extractMentions, MentionInput } from "./MentionInput";
import { Button } from "./ui";

/** Inline composer with the local/github mode toggle, for diff lines:
 * draft a local thread, or drop a raw review comment straight on the
 * line and discuss it locally after it syncs back. */
export function InlineComposerSection(props: {
  path: string;
  line: number;
  side: DiffSide;
  endLine?: number | undefined;
  onEndLineChange?: ((end: number) => void) | undefined;
  /** Existing GitHub review comment this composer hangs off. */
  githubCommentId?: number | undefined;
  onDone: () => void;
}): ReactElement {
  const [mode, setMode] = useState<"local" | "github">("local");
  return (
    <div>
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
            {m === "local" ? "comment locally" : "comment on github"}
          </button>
        ))}
        <span className="ml-auto text-[10px] text-muted">
          {mode === "local"
            ? "stays in Tandem until you post it"
            : "posts inline on this line after confirm"}
        </span>
      </div>
      {mode === "local" ? (
        <InlineCommentForm
          path={props.path}
          line={props.line}
          side={props.side}
          endLine={props.endLine}
          onEndLineChange={props.onEndLineChange}
          githubCommentId={props.githubCommentId}
          onDone={props.onDone}
        />
      ) : (
        <GithubReplyComposer
          reviewCommentId={props.githubCommentId}
          anchor={
            props.githubCommentId === undefined
              ? { path: props.path, line: props.line, sideNew: props.side === "new" }
              : undefined
          }
          onDone={props.onDone}
        />
      )}
    </div>
  );
}

export function InlineCommentForm(props: {
  path: string;
  line: number;
  side: DiffSide;
  onDone: () => void;
  endLine?: number | undefined;
  onEndLineChange?: ((end: number) => void) | undefined;
  githubCommentId?: number | undefined;
}): ReactElement | null {
  const [body, setBody] = useState("");
  const [severity, setSeverity] = useState<CommentSeverity>("suggestion");
  const bundle = useAppStore((s) => s.bundle);
  const specs = useAppStore((s) => s.agentSpecs);
  const addComment = useAppStore((s) => s.addComment);
  const mentionAgent = useAppStore((s) => s.mentionAgent);

  if (!bundle) return null;
  const pr = bundle.detail.pull_request;
  const endLine = props.endLine ?? props.line;

  const submit = () => {
    const text = body.trim();
    if (!text) return;
    const mentions = extractMentions(
      text,
      specs.map((sp) => sp.name),
    );
    void addComment({
      repo: pr.repo,
      pr_number: pr.number,
      head_sha: pr.head_sha,
      path: props.path,
      side: props.side,
      line: props.line,
      end_line: endLine > props.line ? endLine : null,
      body: text,
      suggestion: null,
      tag: null,
      author_kind: "human",
      author_name: "you",
      severity,
      run_id: null,
      parent_id: null,
      github_comment_id: props.githubCommentId ?? null,
    }).then(async (created) => {
      if (created) {
        for (const name of mentions) await mentionAgent(name, created.id);
      }
      props.onDone();
    });
  };

  return (
    <div className="flex flex-col gap-2">
      <MentionInput
        value={body}
        onChange={setBody}
        onSubmit={submit}
        onCancel={props.onDone}
        mono
        placeholder={`local comment on ${props.path}:${String(props.line)}${
          endLine > props.line ? `–${String(endLine)}` : ""
        } — @mention an agent to investigate (⌘↵ to save)`}
      />
      <div className="flex flex-wrap items-center gap-2">
        <select
          value={severity}
          onChange={(e) => {
            setSeverity(e.target.value as CommentSeverity);
          }}
          className="rounded-md border border-edge bg-panel-2 px-2 py-1 text-xs text-cream"
        >
          <option value="info">info</option>
          <option value="suggestion">suggestion</option>
          <option value="issue">issue</option>
          <option value="blocker">blocker</option>
        </select>
        {props.onEndLineChange ? (
          <label className="flex items-center gap-1 text-[11px] text-muted">
            to line
            <input
              type="number"
              min={props.line}
              value={endLine}
              onChange={(e) => {
                props.onEndLineChange?.(Math.max(props.line, Number(e.target.value) || props.line));
              }}
              className="w-16 rounded-md border border-edge bg-panel-2 px-1.5 py-1 text-xs text-cream"
            />
          </label>
        ) : null}
        <Button kind="primary" onClick={submit} disabled={!body.trim()}>
          add local comment
        </Button>
        <Button onClick={props.onDone}>cancel</Button>
      </div>
    </div>
  );
}
