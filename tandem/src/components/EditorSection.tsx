// Settings for "open in editor": the editor command and per-repo
// working-tree paths (blank = Tandem manages a partial clone).

import type { ReactElement } from "react";
import type { Settings } from "../lib/types";
import { useAppStore } from "../state/store";

const inputClass =
  "w-full rounded-md border border-edge bg-ground px-2 py-1.5 text-xs text-cream outline-none focus:border-sky";

export function EditorSection({ settings }: { settings: Settings }): ReactElement {
  const saveSettings = useAppStore((s) => s.saveSettings);
  const orgRepos = useAppStore((s) => s.orgRepos);

  const saveCommand = (editor_command: string) => {
    void saveSettings({ ...settings, editor_command });
  };
  const savePath = (slug: string, path: string) => {
    const repo_paths = { ...settings.repo_paths };
    if (path.trim()) repo_paths[slug] = path.trim();
    else delete repo_paths[slug]; // eslint-disable-line @typescript-eslint/no-dynamic-delete
    void saveSettings({ ...settings, repo_paths });
  };

  return (
    <section className="rounded-lg border border-edge bg-panel p-4">
      <h2 className="text-sm font-semibold text-cream">Editor &amp; working trees</h2>
      <p className="mt-0.5 text-xs text-muted">
        &quot;open in editor&quot; on a PR uses this command. Point repos at your own checkouts
        (never touched by Tandem), or leave blank and Tandem keeps a managed clone with the PR
        branch checked out.
      </p>
      <div className="mt-3 flex flex-col gap-2.5 text-xs">
        <label className="flex flex-col gap-1">
          <span className="text-muted">
            editor command — e.g. <code>code</code>, <code>cursor</code>, <code>zed</code>,{" "}
            <code>subl</code> (terminal editors like nvim need a wrapper that opens a terminal)
          </span>
          <input
            defaultValue={settings.editor_command}
            onBlur={(e) => {
              if (e.target.value !== settings.editor_command) saveCommand(e.target.value);
            }}
            placeholder="code"
            className={`${inputClass} font-mono`}
          />
        </label>
        {orgRepos.length > 0 ? (
          <div className="flex flex-col gap-1.5">
            <span className="text-muted">working tree per repo (blank = managed clone)</span>
            <div className="flex max-h-72 flex-col gap-1.5 overflow-y-auto pr-1">
              {orgRepos.map((repo) => (
                <label key={repo.full_name} className="flex items-center gap-2">
                  <span className="w-56 shrink-0 truncate font-mono text-muted">
                    {repo.full_name}
                  </span>
                  <input
                    defaultValue={settings.repo_paths[repo.full_name] ?? ""}
                    onBlur={(e) => {
                      if (e.target.value !== (settings.repo_paths[repo.full_name] ?? ""))
                        savePath(repo.full_name, e.target.value);
                    }}
                    placeholder="~/GitHub/… (optional)"
                    className={`${inputClass} font-mono`}
                  />
                </label>
              ))}
            </div>
          </div>
        ) : null}
      </div>
    </section>
  );
}
