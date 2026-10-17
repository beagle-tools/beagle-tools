// The app's top bar: home mark, back/forward, and the global buttons
// (updates, review status, agents, search, notifications, theme,
// shortcuts, settings, zoom). Split out of App.tsx to honor the
// 400-line rule.

import {
  ArrowLeft,
  ArrowRight,
  Inbox,
  Keyboard,
  Moon,
  Search,
  Settings,
  Sun,
  ZoomIn,
  ZoomOut,
} from "lucide-react";
import type { ReactElement } from "react";

import { AgentsMenuButton } from "./AgentsDashboard";
import { NotificationsBell } from "./NotificationsBell";
import { usePrSearch } from "./PrSearchPane";
import { useQueueCounts } from "../state/counts";
import { IconButton, TandemMark } from "./ui";
import { UpdateButton } from "./UpdateButton";
import { useKeyNav } from "../state/keyNav";
import { goBack, goForward, useCanGoBack, useCanGoForward } from "../state/navHistory";
import { useAppStore } from "../state/store";

/** The header bar. `zoom` is the current UI zoom percentage; `changeZoom`
 * takes a delta in percentage points, with 0 meaning "reset to 100%". */
export function AppHeader({
  zoom,
  changeZoom,
}: {
  zoom: number;
  changeZoom: (delta: number) => void;
}): ReactElement {
  const view = useAppStore((s) => s.view);
  const setView = useAppStore((s) => s.setView);
  const goHome = useAppStore((s) => s.goHome);
  const selectedPr = useAppStore((s) => s.selectedPr);
  const theme = useAppStore((s) => s.theme);
  const toggleTheme = useAppStore((s) => s.toggleTheme);

  return (
    <div className="flex items-center gap-2.5 border-b border-edge bg-panel px-4 py-2">
      <button
        type="button"
        onClick={goHome}
        title="back to review status"
        className="flex items-center gap-2.5 rounded-lg px-1 py-0.5 transition-colors hover:bg-panel-2"
      >
        <span className="flex size-7 items-center justify-center rounded-lg bg-gradient-to-br from-sky-deep to-sky text-white shadow-sm">
          <TandemMark size={16} />
        </span>
        <span className="text-sm font-semibold tracking-wide text-cream">Tandem</span>
      </button>
      <BackButton />
      <ForwardButton />
      <div className="ml-auto flex items-center gap-1.5">
        <UpdateButton />
        <ReviewStatusMenuButton />
        <AgentsMenuButton />
        <PrSearchButton />
        <NotificationsBell />
        <IconButton onClick={toggleTheme} title="toggle light/dark theme">
          {theme === "dark" ? <Sun size={15} /> : <Moon size={15} />}
        </IconButton>
        <IconButton
          onClick={() => {
            useKeyNav.getState().setHelp(true);
          }}
          title="keyboard shortcuts (?)"
        >
          <Keyboard size={15} />
        </IconButton>
        <IconButton
          onClick={() => {
            setView(
              view === "settings" ? (selectedPr === null ? "review-status" : "review") : "settings",
            );
          }}
          title={view === "settings" ? "back" : "settings"}
          active={view === "settings"}
        >
          {view === "settings" ? <ArrowLeft size={15} /> : <Settings size={15} />}
        </IconButton>
        <span className="flex items-center gap-0.5 rounded-lg bg-panel-2/60 px-1">
          <IconButton
            onClick={() => {
              changeZoom(-10);
            }}
            title="zoom out (⌘-)"
          >
            <ZoomOut size={13} />
          </IconButton>
          <button
            type="button"
            onClick={() => {
              changeZoom(0);
            }}
            title="reset zoom (⌘0)"
            className="w-9 text-center text-[10px] text-muted transition-colors hover:text-cream"
          >
            {zoom}%
          </button>
          <IconButton
            onClick={() => {
              changeZoom(10);
            }}
            title="zoom in (⌘+)"
          >
            <ZoomIn size={13} />
          </IconButton>
        </span>
      </div>
    </div>
  );
}

/** Step back out of a detour, up to five places deep. */
function BackButton() {
  const canGoBack = useCanGoBack();
  return (
    <IconButton
      onClick={() => {
        void goBack();
      }}
      title={canGoBack ? "back (\u2318[)" : "nowhere to go back to"}
      disabled={!canGoBack}
    >
      <ArrowLeft size={15} />
    </IconButton>
  );
}

/** Retrace a step you took back. */
function ForwardButton() {
  const canGoForward = useCanGoForward();
  return (
    <IconButton
      onClick={() => {
        void goForward();
      }}
      title={canGoForward ? "forward (\u2318])" : "nowhere to go forward to"}
      disabled={!canGoForward}
    >
      <ArrowRight size={15} />
    </IconButton>
  );
}

/** Open the find-a-PR pane from anywhere. */
function PrSearchButton() {
  const open = usePrSearch((s) => s.open);
  const setOpen = usePrSearch((s) => s.setOpen);
  return (
    <IconButton
      onClick={() => {
        setOpen(!open);
      }}
      title="search pull requests (\u2318F)"
      active={open}
    >
      <Search size={15} />
    </IconButton>
  );
}

/** Header nav button: jumps to the review-status page, badged with how
 * many reviews are pending across every tracked org. */
export function ReviewStatusMenuButton(): ReactElement {
  const count = useQueueCounts().total;
  const view = useAppStore((s) => s.view);
  const selectedPr = useAppStore((s) => s.selectedPr);
  const setView = useAppStore((s) => s.setView);

  return (
    <IconButton
      onClick={() => {
        // Only toggles back to the review view when there's a PR open
        // to go back to — otherwise this is home and stays put.
        setView(view === "review-status" && selectedPr !== null ? "review" : "review-status");
      }}
      title="review status — PRs across your orgs needing your review"
      active={view === "review-status"}
    >
      <span className="relative">
        <Inbox size={15} />
        {count > 0 ? (
          <span className="absolute -right-1.5 -top-1.5 flex size-3.5 items-center justify-center rounded-full bg-amber text-[8px] font-bold text-ground">
            {count > 9 ? "9+" : count}
          </span>
        ) : null}
      </span>
    </IconButton>
  );
}
