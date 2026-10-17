import { describe, expect, it } from "vitest";

import type { AppRelease } from "../types";
import { compareVersions, publishedDay, releaseAction, sortReleases, tagVersion } from "./versions";

function release(version: string, overrides: Partial<AppRelease> = {}): AppRelease {
  return {
    version,
    tag: `beagle-rca-v${version}`,
    published_at: "2026-10-02T12:00:00Z",
    installable: true,
    ...overrides,
  };
}

describe("tagVersion", () => {
  it("accepts rca release tags", () => {
    expect(tagVersion("beagle-rca-v0.29.0")).toBe("0.29.0");
    expect(tagVersion("beagle-rca-v10.20.30")).toBe("10.20.30");
  });

  it("rejects other apps, mirrors, legacy and malformed tags", () => {
    for (const tag of [
      "beagle-rca-latest",
      "beagle-tandem-v1.2.3",
      "beagle-workflows-v1.2.3",
      "desktop-v0.2.0",
      "v0.29.0",
      "beagle-rca-v1.2",
      "beagle-rca-v1.2.3.4",
      "beagle-rca-v1.2.3-rc1",
      "beagle-rca-v1.x.3",
    ]) {
      expect(tagVersion(tag), tag).toBeNull();
    }
  });
});

describe("compareVersions", () => {
  it("compares numerically, not lexically", () => {
    expect(compareVersions("0.10.0", "0.9.9")).toBeGreaterThan(0);
    expect(compareVersions("1.0.0", "1.0.0")).toBe(0);
    expect(compareVersions("0.2.0", "0.2.1")).toBeLessThan(0);
  });
});

describe("releaseAction", () => {
  it("marks the running version current", () => {
    expect(releaseAction(release("0.29.0"), "0.29.0")).toBe("current");
  });

  it("offers update for newer and rollback for older releases", () => {
    expect(releaseAction(release("0.30.0"), "0.29.0")).toBe("update");
    expect(releaseAction(release("0.28.3"), "0.29.0")).toBe("rollback");
  });

  it("never offers a release without an updater manifest", () => {
    expect(releaseAction(release("0.30.0", { installable: false }), "0.29.0")).toBe("unavailable");
  });
});

describe("sortReleases", () => {
  it("sorts newest first and drops tags that don't match their version", () => {
    const sorted = sortReleases([
      release("0.9.0"),
      release("0.10.0"),
      release("1.0.0", { tag: "beagle-tandem-v1.0.0" }),
      release("0.9.1"),
    ]);
    expect(sorted.map((r) => r.version)).toEqual(["0.10.0", "0.9.1", "0.9.0"]);
  });
});

describe("publishedDay", () => {
  it("keeps the date part, or nothing when unknown", () => {
    expect(publishedDay(release("1.0.0"))).toBe("2026-10-02");
    expect(publishedDay(release("1.0.0", { published_at: null }))).toBe("");
  });
});
