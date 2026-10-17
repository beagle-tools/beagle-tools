// Org lookup for settings: browse the orgs you belong to (plus your own
// account) and track one with a click. Repos themselves are never
// picked here — the home-screen repo selector draws live from every
// tracked org.

import { Plus, Search } from "lucide-react";
import { useState } from "react";
import type { ReactElement } from "react";

import { ipc } from "../lib/ipc";
import type { OwnerList } from "../lib/types";
import { useAppStore } from "../state/store";
import { Button, Spinner } from "./ui";

export function OrgBrowser(): ReactElement | null {
  const settings = useAppStore((s) => s.settings);
  const saveSettings = useAppStore((s) => s.saveSettings);
  const [owners, setOwners] = useState<OwnerList | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (!settings) return null;

  const open = () => {
    setLoading(true);
    setError(null);
    ipc
      .listGithubOwners()
      .then(setOwners)
      .catch((e: unknown) => {
        setError(e instanceof Error ? e.message : String(e));
      })
      .finally(() => {
        setLoading(false);
      });
  };

  if (!owners) {
    return (
      <div className="flex items-center gap-2">
        <Button onClick={open} disabled={loading}>
          <Search size={11} /> browse github
        </Button>
        {loading ? <Spinner label="loading your orgs…" /> : null}
        {error ? <span className="text-[11px] text-ember">{error}</span> : null}
      </div>
    );
  }

  const candidates = [owners.viewer, ...owners.orgs].filter((o) => !settings.orgs.includes(o));

  return (
    <div className="flex flex-col gap-2 rounded-md border border-edge bg-ground/60 p-2.5">
      {error ? <div className="text-[11px] text-ember">{error}</div> : null}
      <div className="max-h-56 overflow-y-auto rounded-md">
        {candidates.map((owner) => (
          <div
            key={owner}
            className="flex items-center gap-2 rounded-md px-2 py-1.5 text-xs transition-colors hover:bg-panel-2"
          >
            <span className="truncate font-mono text-cream">{owner}</span>
            {owner === owners.viewer ? <span className="text-[10px] text-muted">(you)</span> : null}
            <span className="ml-auto shrink-0">
              <Button
                onClick={() => {
                  void saveSettings({ ...settings, orgs: [...settings.orgs, owner] });
                }}
              >
                <Plus size={11} /> track
              </Button>
            </span>
          </div>
        ))}
        {candidates.length === 0 ? (
          <div className="px-2 py-3 text-center text-[11px] text-muted">
            every org you belong to is already tracked
          </div>
        ) : null}
      </div>
    </div>
  );
}
