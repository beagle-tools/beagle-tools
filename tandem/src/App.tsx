// App shell: header, the right-hand pane, and the main views. Review
// status is home; the pull-request review view is what opening a PR
// from it leads to, and settings sits behind the gear.

import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useState } from "react";
import type { ReactElement } from "react";

import { AgentsDashboard } from "./components/AgentsDashboard";
import { AppHeader } from "./components/AppHeader";
import { PrSearchPane } from "./components/PrSearchPane";
import { ReviewLayout } from "./components/ReviewLayout";
import { ReviewStatusView } from "./components/ReviewStatusView";
import { SettingsView } from "./components/SettingsView";
import { ShortcutManager } from "./components/ShortcutManager";
import { SummaryPane } from "./components/SummarySection";
import { ThreadPane } from "./components/ThreadPane";
import { ToastHost } from "./components/ToastHost";
import { applyZoom, loadZoom } from "./state/persist";
import { useAppStore } from "./state/store";

export default function App(): ReactElement {
  const init = useAppStore((s) => s.init);
  const view = useAppStore((s) => s.view);

  const [zoom, setZoom] = useState(loadZoom);
  const changeZoom = (delta: number) => {
    setZoom((z) => {
      const next = delta === 0 ? 100 : Math.min(160, Math.max(70, z + delta));
      applyZoom(next);
      return next;
    });
  };

  useEffect(() => {
    void init();
    // Bring the window to the front on startup — after an update the
    // relaunched app would otherwise reopen behind other windows.
    getCurrentWindow()
      .setFocus()
      .catch(() => undefined);
    applyZoom(loadZoom());
    const onKey = (e: KeyboardEvent) => {
      if (!(e.metaKey || e.ctrlKey)) return;
      if (e.key === "=" || e.key === "+") {
        e.preventDefault();
        changeZoom(10);
      } else if (e.key === "-") {
        e.preventDefault();
        changeZoom(-10);
      } else if (e.key === "0") {
        e.preventDefault();
        changeZoom(0);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
    };
  }, [init]);

  return (
    <div className="flex h-full flex-col">
      <AppHeader zoom={zoom} changeZoom={changeZoom} />

      <ToastHost />
      <ShortcutManager />
      <PrSearchPane />
      <ThreadPane />
      <SummaryPane />
      <div className="flex min-h-0 flex-1">
        {view === "settings" ? (
          <main className="animate-fade-up flex-1 overflow-y-scroll [scrollbar-gutter:stable]">
            <SettingsView />
          </main>
        ) : view === "agents" ? (
          <main className="animate-fade-up flex-1 overflow-y-scroll [scrollbar-gutter:stable]">
            <AgentsDashboard />
          </main>
        ) : view === "review-status" ? (
          <main className="animate-fade-up flex-1 overflow-y-scroll [scrollbar-gutter:stable]">
            <ReviewStatusView />
          </main>
        ) : (
          <ReviewLayout />
        )}
      </div>
    </div>
  );
}
