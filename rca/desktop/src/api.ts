// Typed wrappers over the Tauri invoke surface. Every command the
// frontend calls lives here — components never import `invoke` directly,
// so the IPC surface stays greppable in one file.

import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";

import type { CorpusLine } from "./lib/finder";
import type { AgentsEvent, AgentsStatus, AppRelease, Listing } from "./types";

/** Active + archived workspaces plus load problems, sidebar-sorted. */
export async function listWorkspaces(): Promise<Listing> {
  return invoke<Listing>("list_workspaces");
}

/**
 * One section's raw markdown, or `null` when the file doesn't exist yet
 * (a normal state — the investigation just hasn't got there).
 */
export async function readSection(id: string, file: string): Promise<string | null> {
  return invoke<string | null>("read_section", { id, file });
}

/** The workspace's diagram file names, sorted. */
export async function listDiagrams(id: string): Promise<string[]> {
  return invoke<string[]>("list_diagrams", { id });
}

/** One diagram's text, ANSI-stripped; `null` if it vanished. */
export async function readDiagram(id: string, name: string): Promise<string | null> {
  return invoke<string | null>("read_diagram", { id, name });
}

/** Archives a finished workspace; errors explain the sign-off path. */
export async function archiveWorkspace(id: string): Promise<string> {
  return invoke<string>("archive_workspace", { id });
}

/** Moves an archived workspace back to the active list. */
export async function unarchiveWorkspace(id: string): Promise<string> {
  return invoke<string>("unarchive_workspace", { id });
}

/** Every searchable line across all workspaces — the finder's corpus. */
export async function searchCorpus(): Promise<CorpusLine[]> {
  return invoke<CorpusLine[]>("search_corpus");
}

/** Attaches a remediation PR URL; false when it was already attached. */
export async function addPr(id: string, url: string): Promise<boolean> {
  return invoke<boolean>("add_pr", { id, url });
}

/** Live PR states via gh: url → "open" | "draft" | "merged" | "closed". */
export async function prStates(urls: string[]): Promise<Record<string, string>> {
  return invoke<Record<string, string>>("pr_states", { urls });
}

/** Opens a URL in the system browser. */
export async function openInBrowser(url: string): Promise<void> {
  return openUrl(url);
}

// The beagle-agentd control surface.

/** One-shot daemon status snapshot; throws when the daemon isn't reachable. */
export async function agentsStatus(): Promise<AgentsStatus> {
  return invoke<AgentsStatus>("agents_status");
}

/** Resumes ticking an agent. */
export async function startAgent(id: string): Promise<void> {
  await invoke("start_agent", { id });
}

/** Pauses an agent. */
export async function stopAgent(id: string): Promise<void> {
  await invoke("stop_agent", { id });
}

/** Asks the daemon to re-read its config. */
export async function reloadAgentsConfig(): Promise<void> {
  await invoke("reload_agents_config");
}

/**
 * Subscribes to live daemon status pushes. The callback fires on every update
 * (and on disconnect, with `connected: false`). Returns an unlisten function.
 */
export async function onAgentsStatus(handler: (event: AgentsEvent) => void): Promise<() => void> {
  return listen<AgentsEvent>("agents-status", (event) => {
    handler(event.payload);
  });
}

// The in-app updater. The routine check reads the `beagle-rca-latest`
// manifest configured in tauri.conf.json; installing a specific version
// (rollback) goes through the Rust side, which points the updater at that
// release's own manifest. Every bundle is signature-checked either way.

let pendingUpdate: Update | null = null;

/** The running app's version (tauri.conf.json `version`). */
export async function appVersion(): Promise<string> {
  return getVersion();
}

/**
 * Asks the release feed whether a newer version exists. Returns it, or
 * `null` when up to date — and also when the check fails (dev builds,
 * offline): an update check is never worth an error banner.
 */
export async function checkForUpdate(): Promise<string | null> {
  try {
    const update = await check();
    if (update === null) {
      return null;
    }
    pendingUpdate = update;
    return update.version;
  } catch (cause: unknown) {
    console.warn("update check skipped", cause);
    return null;
  }
}

/** Downloads + installs the update `checkForUpdate` found, then restarts. */
export async function installUpdateAndRelaunch(): Promise<void> {
  if (pendingUpdate === null) {
    throw new Error("no update pending — check for updates first");
  }
  try {
    await pendingUpdate.downloadAndInstall();
  } catch (cause: unknown) {
    const message = cause instanceof Error ? cause.message : String(cause);
    throw new Error(`update failed: ${message}`, { cause });
  }
  await relaunch();
}

/** Every published rca release (`beagle-rca-v*`), newest first. */
export async function listAppReleases(): Promise<AppRelease[]> {
  return invoke<AppRelease[]>("list_app_releases");
}

/** Installs one specific release (upgrade or rollback), then restarts. */
export async function installAppVersion(tag: string): Promise<void> {
  await invoke("install_app_version", { tag });
  await relaunch();
}
