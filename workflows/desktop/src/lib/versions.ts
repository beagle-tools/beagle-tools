// Pure version arithmetic for the Versions dialog. No Tauri, so it is
// testable on its own.

import type { AppRelease } from "./types";

/** Numeric `X.Y.Z` ordering: negative when a < b, 0 when equal. */
export function compareVersions(a: string, b: string): number {
  const pa = a.split(".").map(Number);
  const pb = b.split(".").map(Number);
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    const d = (pa[i] ?? 0) - (pb[i] ?? 0);
    if (d !== 0) return d;
  }
  return 0;
}

/** What the row for `release` offers when `current` is running. */
export type ReleaseAction = "current" | "update" | "rollback" | "unavailable";

export function releaseAction(release: AppRelease, current: string): ReleaseAction {
  if (release.version === current) return "current";
  if (!release.installable) return "unavailable";
  return compareVersions(release.version, current) > 0 ? "update" : "rollback";
}
