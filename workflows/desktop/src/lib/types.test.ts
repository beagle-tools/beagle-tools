import { describe, expect, it } from "vitest";

import { isActive } from "./types";

describe("isActive", () => {
  it("is true only for states that can still change", () => {
    expect(isActive("pending")).toBe(true);
    expect(isActive("running")).toBe(true);
    for (const s of ["succeeded", "failed", "timedout", "killed"] as const) {
      expect(isActive(s)).toBe(false);
    }
  });
});
