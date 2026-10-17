import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { age, cost, duration, params } from "./format";

const NOW = new Date("2026-01-01T12:00:00Z");

describe("duration", () => {
  it("is a dash without a start or with an unparseable timestamp", () => {
    expect(duration(null, null)).toBe("—");
    expect(duration("not a date", "2026-01-01T00:00:00Z")).toBe("—");
  });

  it("keeps at most two units and drops zero remainders", () => {
    const start = "2026-01-01T00:00:00Z";
    expect(duration(start, "2026-01-01T00:00:45Z")).toBe("45s");
    expect(duration(start, "2026-01-01T00:03:00Z")).toBe("3m");
    expect(duration(start, "2026-01-01T00:03:12Z")).toBe("3m 12s");
    expect(duration(start, "2026-01-01T01:04:12Z")).toBe("1h 4m");
    expect(duration(start, "2026-01-01T02:00:59Z")).toBe("2h");
    expect(duration(start, "2026-01-03T05:00:00Z")).toBe("2d 5h");
    expect(duration(start, "2026-01-02T00:30:00Z")).toBe("1d");
  });

  it("clamps a finish before the start to zero", () => {
    expect(duration("2026-01-01T00:01:00Z", "2026-01-01T00:00:00Z")).toBe("0s");
  });
});

describe("clock-relative helpers", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(NOW);
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("age measures from now", () => {
    expect(age(null)).toBe("—");
    expect(age("garbage")).toBe("—");
    expect(age("2026-01-01T11:55:00Z")).toBe("5m");
    // A timestamp from the future (clock skew) is not negative.
    expect(age("2026-01-01T13:00:00Z")).toBe("0s");
  });

  it("duration of an unfinished run runs to now", () => {
    expect(duration("2026-01-01T11:00:00Z", null)).toBe("1h");
  });
});

describe("cost", () => {
  it("formats dollars with two decimals", () => {
    expect(cost(null)).toBe("—");
    expect(cost(0)).toBe("$0.00");
    expect(cost(1.234)).toBe("$1.23");
  });

  it("never shows a sub-cent run as free", () => {
    expect(cost(0.004)).toBe("<$0.01");
  });
});

describe("params", () => {
  it("keeps only string values from a JSON object", () => {
    expect(params('{"ticket":"EI-1","n":3,"x":null}')).toEqual({ ticket: "EI-1" });
  });

  it("is empty for missing, malformed, or non-object input", () => {
    expect(params(null)).toEqual({});
    expect(params("{nope")).toEqual({});
    expect(params('"str"')).toEqual({});
    expect(params("null")).toEqual({});
  });
});
