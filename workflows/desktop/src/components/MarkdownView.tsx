import { useEffect, useState, type ReactElement } from "react";
import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";

import { errorMessage, ipc } from "../lib/ipc";

// The write-ups are the reason this app is worth having over `tail -f`:
// they are prose meant to be read, so they get rendered, not shown as
// source. Same decision as `ag -i` piping through glow rather than bat.
export function MarkdownView({ path }: { path: string }): ReactElement {
  const [text, setText] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setText(null);
    setError(null);
    ipc
      .readMarkdown(path)
      .then((t) => {
        if (!cancelled) setText(t);
      })
      .catch((e: unknown) => {
        if (!cancelled) setError(errorMessage(e));
      });
    return () => {
      cancelled = true;
    };
  }, [path]);

  if (error) return <div className="empty muted">{error}</div>;
  if (text === null) return <div className="empty muted">Reading…</div>;
  return (
    <div className="markdown">
      <Markdown remarkPlugins={[remarkGfm]}>{text}</Markdown>
      <div className="pathline muted small">{path}</div>
    </div>
  );
}
