// Pure version logic for the in-app updater: comparing X.Y.Z versions,
// recognising rca's release tags in the monorepo feed, and deciding what
// the version list offers for each release. DOM- and IPC-free.

import type { AppRelease } from "../types";

/** rca's versioned release tags: `beagle-rca-vX.Y.Z`. */
export const TAG_PREFIX = "beagle-rca-v";

const SEMVER = /^(\d+)\.(\d+)\.(\d+)$/;

/** The `X.Y.Z` an rca release tag carries, or `null` for any other tag. */
export function tagVersion(tag: string): string | null {
  if (!tag.startsWith(TAG_PREFIX)) {
    return null;
  }
  const version = tag.slice(TAG_PREFIX.length);
  return SEMVER.test(version) ? version : null;
}

/** Numeric X.Y.Z comparison: negative when `a` is older than `b`. */
export function compareVersions(a: string, b: string): number {
  const pa = a.split(".").map(Number);
  const pb = b.split(".").map(Number);
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    const diff = (pa[i] ?? 0) - (pb[i] ?? 0);
    if (diff !== 0) {
      return diff;
    }
  }
  return 0;
}

/** What the version list offers for one release, relative to `current`. */
export type ReleaseAction = "current" | "update" | "rollback" | "unavailable";

export function releaseAction(release: AppRelease, current: string): ReleaseAction {
  if (release.version === current) {
    return "current";
  }
  if (!release.installable) {
    return "unavailable";
  }
  return compareVersions(release.version, current) > 0 ? "update" : "rollback";
}

/**
 * Keeps only installable-looking rca releases (defensive: the Rust side
 * already filters) and sorts newest first.
 */
export function sortReleases(releases: AppRelease[]): AppRelease[] {
  return releases
    .filter((release) => tagVersion(release.tag) === release.version)
    .sort((a, b) => compareVersions(b.version, a.version));
}

/** `2026-10-02T12:00:00Z` → `2026-10-02`; empty when unknown. */
export function publishedDay(release: AppRelease): string {
  return release.published_at?.slice(0, 10) ?? "";
}
