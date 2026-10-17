// App self-update (via the Tauri updater plugin). Release CI (see the repo's
// docs/SDLC.md) publishes signed update bundles plus a latest.json manifest
// to the rolling `beagle-tandem-latest` release; the plugin checks that
// manifest against the running version.

import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";

let pending: Update | null = null;

/**
 * Ask GitHub whether a newer release exists. Returns the new version
 * string, or null when up to date (or when the check fails — e.g. dev
 * builds or offline; updates are never worth an error bar).
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

/**
 * Download + install the update found by `checkForUpdate`, then restart
 * the app. Only throws with a user-readable message.
 */
export async function installUpdateAndRelaunch(): Promise<void> {
  if (pending === null) throw new Error("no update pending — check for updates first");
  try {
    await pending.downloadAndInstall();
    await relaunch();
  } catch (e: unknown) {
    throw new Error(`update failed: ${e instanceof Error ? e.message : String(e)}`);
  }
}
