// "Open in editor": resolve (or create) this PR's working tree and
// launch the configured editor on it. The tooltip shows where it will
// land; managed clones get the PR branch checked out, user checkouts
// are never touched.

import { Code2 } from "lucide-react";
import { useEffect, useState } from "react";
import type { ReactElement } from "react";

import { ipc } from "../lib/ipc";
import type { WorkspaceInfo } from "../lib/types";
import { pushGithubError, pushInfo } from "../state/toasts";
import { Button, Spinner } from "./ui";

export function OpenInEditorButton({
  repo,
  number,
}: {
  repo: string;
  number: number;
}): ReactElement {
  const [info, setInfo] = useState<WorkspaceInfo | null>(null);
  const [working, setWorking] = useState(false);

  useEffect(() => {
    setInfo(null);
    ipc.resolveWorkspace(repo).then(setInfo).catch(console.warn);
  }, [repo]);

  const title = info
    ? info.exists
      ? `open ${info.path} in your editor${info.managed ? " (managed clone — checks out the PR branch)" : " (your checkout — left untouched)"}`
      : `clone into ${info.path}, check out this PR, and open your editor`
    : "open this PR's working tree in your editor";

  const open = () => {
    setWorking(true);
    ipc
      .openInEditor(repo, number)
      .then((path) => {
        pushInfo(`opened in editor: ${path}`);
        ipc.resolveWorkspace(repo).then(setInfo).catch(console.warn);
      })
      .catch((e: unknown) => {
        pushGithubError(e instanceof Error ? e.message : String(e));
      })
      .finally(() => {
        setWorking(false);
      });
  };

  if (working) {
    return <Spinner label={info?.exists === false ? "cloning…" : "opening…"} />;
  }
  return (
    <Button onClick={open} title={title}>
      <Code2 size={12} /> editor
    </Button>
  );
}
