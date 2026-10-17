// Browser-side stand-in for the Tauri backend during Playwright runs: every
// `invoke` the app makes is answered from fixtures.ts. Loaded only by
// `vite --mode e2e` (see main.tsx), so it never ships in a production build.
//
// Specs steer it through `window.__BEAGLE_E2E__` (set with
// page.addInitScript before the app boots), e.g. `{ update: "0.30.0" }` to
// make the updater report a newer release.

import { mockIPC } from "@tauri-apps/api/mocks";

import { AGENTS, CORPUS, DIAGRAMS, LISTING, PR_URL, RELEASES, SECTIONS } from "./fixtures";

/** Knobs a spec can set before the app loads. */
export interface E2eOptions {
  /** A newer version for the updater check to report; none by default. */
  update?: string;
}

declare global {
  interface Window {
    __BEAGLE_E2E__?: E2eOptions;
  }
}

const APP_VERSION = "0.29.0";

/** Reads a string field from the untyped invoke payload. */
function arg(payload: unknown, key: string): string {
  if (typeof payload === "object" && payload !== null && key in payload) {
    const value: unknown = (payload as Record<string, unknown>)[key];
    return typeof value === "string" ? value : "";
  }
  return "";
}

function updateMetadata(version: string): Record<string, unknown> {
  return {
    rid: 1,
    currentVersion: APP_VERSION,
    version,
    date: "2026-10-15T12:00:00Z",
    body: "e2e update",
    rawJson: {},
  };
}

function handle(cmd: string, payload: unknown): unknown {
  switch (cmd) {
    case "list_workspaces":
      return LISTING;
    case "read_section":
      return SECTIONS[arg(payload, "id")]?.[arg(payload, "file")] ?? null;
    case "list_diagrams":
      return Object.keys(DIAGRAMS[arg(payload, "id")] ?? {});
    case "read_diagram":
      return DIAGRAMS[arg(payload, "id")]?.[arg(payload, "name")] ?? null;
    case "search_corpus":
      return CORPUS;
    case "pr_states":
      return { [PR_URL]: "merged" };
    case "agents_status":
      return AGENTS;
    case "list_app_releases":
      return RELEASES;
    case "plugin:app|version":
      return APP_VERSION;
    case "plugin:updater|check": {
      const update = window.__BEAGLE_E2E__?.update;
      return update === undefined ? null : updateMetadata(update);
    }
    default:
      // Mutations (archive, add_pr, start/stop agent, installs) succeed
      // quietly; the specs assert on what renders, not on side effects.
      return null;
  }
}

/** Installs the IPC mock. Call before React renders. */
export function installMockIpc(): void {
  mockIPC(handle, { shouldMockEvents: true });
}
