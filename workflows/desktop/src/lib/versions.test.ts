import { describe, expect, it } from "vitest";

import type { AppRelease } from "./types";
import { compareVersions, releaseAction } from "./versions";

function release(version: string, installable = true): AppRelease {
  return { version, tag: `beagle-workflows-v${version}`, published_at: null, installable };
}

describe("compareVersions", () => {
  it("orders numerically, not lexically", () => {
    expect(compareVersions("0.10.0", "0.9.9")).toBeGreaterThan(0);
    expect(compareVersions("1.2.3", "1.2.10")).toBeLessThan(0);
    expect(compareVersions("2.0.0", "2.0.0")).toBe(0);
  });

  it("treats missing parts as zero", () => {
    expect(compareVersions("1.2", "1.2.0")).toBe(0);
  });
});

describe("releaseAction", () => {
  it("marks the running version as current, even when not installable", () => {
    expect(releaseAction(release("0.2.0", false), "0.2.0")).toBe("current");
  });

  it("offers update for newer and rollback for older releases", () => {
    expect(releaseAction(release("0.3.0"), "0.2.0")).toBe("update");
    expect(releaseAction(release("0.1.9"), "0.2.0")).toBe("rollback");
  });

  it("refuses releases without an updater manifest", () => {
    expect(releaseAction(release("0.3.0", false), "0.2.0")).toBe("unavailable");
  });
});
