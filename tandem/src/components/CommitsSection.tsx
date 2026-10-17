// Collapsible list of the PR's commits, fetched lazily on first expand
// (same disclosure style as the description / status sections).

import { GitCommitHorizontal } from "lucide-react";
import { useState } from "react";
import type { ReactElement } from "react";

import { ipc } from "../lib/ipc";
import { openExternal } from "../lib/open";
import { relativeTime } from "../lib/format";
import type { PrCommit, RepoRef } from "../lib/types";
import { Spinner } from "./ui";

export function CommitsSection({ repo, number }: { repo: RepoRef; number: number }): ReactElement {
  const slug = `${repo.owner}/${repo.name}`;
  const [commits, setCommits] = useState<PrCommit[] | null>(null);
  const [failed, setFailed] = useState(false);

  const load = () => {
    if (commits !== null) return;
    ipc
      .listPrCommits(slug, number)
      .then(setCommits)
      .catch(() => {
        setFailed(true);
      });
  };

  return (
    <details
      className="mt-2"
      onToggle={(e) => {
        if (e.currentTarget.open) load();
      }}
    >
      <summary className="flex cursor-pointer items-center gap-1 text-[11px] text-muted hover:text-cream">
        <GitCommitHorizontal size={10} /> commits
        {commits !== null ? ` (${String(commits.length)})` : ""}
      </summary>
      <div className="mt-1.5 flex flex-col gap-1">
        {failed ? (
          <span className="text-[11px] text-ember">couldn&apos;t load commits</span>
        ) : commits === null ? (
          <Spinner label="loading commits…" />
        ) : commits.length === 0 ? (
          <span className="text-[11px] text-muted">no commits</span>
        ) : (
          commits.map((c) => (
            <div key={c.sha} className="flex items-center gap-2 text-[11px]">
              <button
                type="button"
                onClick={() => {
                  openExternal(`https://github.com/${slug}/commit/${c.sha}`);
                }}
                title="open this commit on github.com"
                className="shrink-0 font-mono text-sky hover:underline"
              >
                {c.sha.slice(0, 7)}
              </button>
              <span className="min-w-0 truncate text-cream" title={c.message}>
                {c.message.split("\n")[0]}
              </span>
              <span className="ml-auto shrink-0 text-muted">
                {c.author}
                {c.authored_at ? ` · ${relativeTime(c.authored_at)}` : ""}
              </span>
            </div>
          ))
        )}
      </div>
    </details>
  );
}
