// App-version state: the "update available" pill and the Versions dialog.
// Kept apart from store.ts because none of it is agentd state — it is
// about this app's own binary. Failures to install land in the main error
// bar; a failed background check is silent (see updater.ts).

import { create } from "zustand";

import { errorMessage, ipc } from "../lib/ipc";
import type { AppRelease } from "../lib/types";
import { appVersion, checkForUpdate, installUpdateAndRelaunch, relaunchApp } from "../lib/updater";
import { useStore } from "./store";

interface UpdatesState {
  /** The running version; "" until known. */
  current: string;
  /** A newer version the mirror offers, or null. */
  available: string | null;
  /** True while downloading/installing; the app relaunches after. */
  installing: boolean;
  releases: AppRelease[] | null;
  releasesError: string | null;
  /** The tag being switched to from the Versions dialog. */
  switching: string | null;

  loadCurrent: () => Promise<void>;
  check: () => Promise<void>;
  install: () => Promise<void>;
  loadReleases: () => Promise<void>;
  installVersion: (tag: string) => Promise<void>;
}

const report = (e: unknown): void => {
  useStore.setState({ error: errorMessage(e) });
};

export const useUpdates = create<UpdatesState>((set, get) => ({
  current: "",
  available: null,
  installing: false,
  releases: null,
  releasesError: null,
  switching: null,

  loadCurrent: async () => {
    try {
      set({ current: await appVersion() });
    } catch (e) {
      console.warn("could not read the app version", e);
    }
  },

  check: async () => {
    const version = await checkForUpdate();
    if (version !== null) set({ available: version });
  },

  install: async () => {
    if (get().installing) return;
    set({ installing: true });
    try {
      await installUpdateAndRelaunch();
    } catch (e) {
      set({ installing: false });
      report(e);
    }
  },

  loadReleases: async () => {
    set({ releases: null, releasesError: null });
    try {
      set({ releases: await ipc.listAppReleases() });
    } catch (e) {
      set({ releasesError: errorMessage(e) });
    }
  },

  installVersion: async (tag) => {
    if (get().switching !== null) return;
    set({ switching: tag });
    try {
      await ipc.installAppVersion(tag);
      await relaunchApp();
    } catch (e) {
      set({ switching: null });
      report(e);
    }
  },
}));
