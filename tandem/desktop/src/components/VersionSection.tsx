// Settings › Version: shows the installed app version and every
// published release, with one-click switch to any of them (upgrade or
// rollback — installs are signature-checked either way, then the app
// relaunches). Always the last section on the settings page.

import { getVersion } from "@tauri-apps/api/app";
import { relaunch } from "@tauri-apps/plugin-process";
import { History, RefreshCw } from "lucide-react";
import { useEffect, useState } from "react";
import type { ReactElement } from "react";

import { ipc } from "../lib/ipc";
import type { AppRelease } from "../lib/types";
import { useToasts } from "../state/toasts";
import { Button, Spinner } from "./ui";

function compareVersions(a: string, b: string): number {
  const pa = a.split(".").map(Number);
  const pb = b.split(".").map(Number);
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    const d = (pa[i] ?? 0) - (pb[i] ?? 0);
    if (d !== 0) return d;
  }
  return 0;
}

const PAGE_SIZE = 5;

export function VersionSection(): ReactElement {
  const push = useToasts((s) => s.push);
  const [current, setCurrent] = useState("");
  const [releases, setReleases] = useState<AppRelease[] | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [installing, setInstalling] = useState<string | null>(null);
  const [visible, setVisible] = useState(PAGE_SIZE);

  const load = () => {
    setLoadError(null);
    setVisible(PAGE_SIZE);
    ipc
      .listAppReleases()
      .then(setReleases)
      .catch((e: unknown) => {
        setLoadError(e instanceof Error ? e.message : String(e));
      });
  };

  useEffect(() => {
    getVersion().then(setCurrent).catch(console.warn);
    load();
  }, []);

  const install = (release: AppRelease) => {
    setInstalling(release.tag);
    ipc
      .installAppVersion(release.tag)
      .then(() => relaunch())
      .catch((e: unknown) => {
        setInstalling(null);
        push("error", e instanceof Error ? e.message : String(e));
      });
  };

  return (
    <section className="rounded-lg border border-edge bg-panel p-4">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="flex items-center gap-1.5 text-sm font-semibold text-cream">
            <History size={14} /> Version
          </h2>
          <p className="mt-0.5 text-xs text-muted">
            Tandem <span className="font-mono text-cream">v{current || "…"}</span> — switch to any
            release below; the app restarts into it
          </p>
        </div>
        <Button onClick={load} title="reload the release list">
          <RefreshCw size={12} /> refresh
        </Button>
      </div>
      <div className="mt-3 flex flex-col gap-1.5">
        {releases === null && loadError === null ? <Spinner label="loading releases…" /> : null}
        {loadError !== null ? (
          <div className="rounded-md bg-ember/10 px-2 py-1.5 text-xs text-ember">{loadError}</div>
        ) : null}
        {releases?.length === 0 ? (
          <p className="text-xs text-muted">no releases published yet</p>
        ) : null}
        {releases?.slice(0, visible).map((r) => {
          const isCurrent = r.version === current;
          const cmp = compareVersions(r.version, current);
          return (
            <div
              key={r.tag}
              className="flex items-center justify-between rounded-md bg-panel-2 px-2 py-1.5 text-xs"
            >
              <span className="flex items-center gap-2">
                <span className="font-mono text-cream">{r.tag}</span>
                {isCurrent ? (
                  <span className="rounded-full bg-sky/15 px-1.5 py-0.5 text-[10px] font-semibold text-sky">
                    current
                  </span>
                ) : null}
                {r.published_at ? (
                  <span className="text-muted">{r.published_at.slice(0, 10)}</span>
                ) : null}
              </span>
              {isCurrent ? null : !r.installable ? (
                <span className="text-muted">not installable</span>
              ) : installing === r.tag ? (
                <Spinner label="installing…" />
              ) : (
                <Button
                  kind={cmp > 0 ? "primary" : "ghost"}
                  disabled={installing !== null}
                  onClick={() => {
                    install(r);
                  }}
                  title={`install ${r.tag} and relaunch`}
                >
                  {cmp > 0 ? "update" : "roll back"}
                </Button>
              )}
            </div>
          );
        })}
        {releases !== null && releases.length > visible ? (
          <button
            type="button"
            onClick={() => {
              setVisible((v) => v + PAGE_SIZE);
            }}
            className="self-center rounded-md px-2 py-1 text-[11px] text-muted transition-colors hover:bg-panel-2 hover:text-cream"
          >
            load {Math.min(PAGE_SIZE, releases.length - visible)} more ({releases.length - visible}{" "}
            older)
          </button>
        ) : null}
      </div>
    </section>
  );
}
