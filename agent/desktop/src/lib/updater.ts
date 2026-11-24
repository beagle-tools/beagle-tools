// App self-update through the Tauri updater plugin. Release CI publishes
// signed bundles plus a latest.json to the rolling
// `beagle-workflows-latest` release; the plugin checks that manifest
// (endpoint + pubkey in tauri.conf.json) against the running version.
// Like ipc.ts, this is the only place the plugin APIs appear.

import { getVersion } from "@tauri-apps/api/app";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";

let pending: Update | null = null;

/**
 * Ask the release mirror whether a newer version exists. Returns it, or
 * null when up to date — and also when the check fails (a dev build,
 * offline): a missed update check is never worth an error bar.
 */
export async function checkForUpdate(): Promise<string | null> {
  try {
    const update = await check();
    if (update === null) return null;
    pending = update;
    return update.version;
  } catch (e: unknown) {
    console.warn("update check skipped", e);
    return null;
  }
}

/** Download + install the update `checkForUpdate` found, then restart. */
export async function installUpdateAndRelaunch(): Promise<void> {
  if (pending === null) throw new Error("no update pending — check for updates first");
  try {
    await pending.downloadAndInstall();
    await relaunch();
  } catch (e: unknown) {
    throw new Error(`update failed: ${e instanceof Error ? e.message : String(e)}`);
  }
}

/** Restart into whatever version was just installed. */
export function relaunchApp(): Promise<void> {
  return relaunch();
}

/** The running app's version (tauri.conf.json, stamped at release). */
export function appVersion(): Promise<string> {
  return getVersion();
}
