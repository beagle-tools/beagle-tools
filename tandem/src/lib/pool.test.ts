import { describe, expect, it } from "vitest";

import { runPool } from "./pool";

const tick = () => new Promise<void>((resolve) => setTimeout(resolve, 0));

describe("runPool", () => {
  it("returns results in input order regardless of completion order", async () => {
    const delays = [30, 0, 10];
    const out = await runPool(delays, 3, async (ms) => {
      await new Promise((resolve) => setTimeout(resolve, ms));
      return ms * 2;
    });
    expect(out).toEqual([60, 0, 20]);
  });

  it("never runs more than `limit` tasks at once", async () => {
    let inFlight = 0;
    let peak = 0;
    await runPool([1, 2, 3, 4, 5, 6], 2, async () => {
      inFlight++;
      peak = Math.max(peak, inFlight);
      await tick();
      inFlight--;
    });
    expect(peak).toBe(2);
  });

  it("reports progress after each task", async () => {
    const progress: [number, number][] = [];
    await runPool(
      ["a", "b", "c"],
      1,
      (x) => Promise.resolve(x),
      (done, total) => progress.push([done, total]),
    );
    expect(progress).toEqual([
      [1, 3],
      [2, 3],
      [3, 3],
    ]);
  });

  it("stops starting new tasks once shouldContinue is false", async () => {
    const started: number[] = [];
    await runPool(
      [1, 2, 3, 4],
      1,
      (n) => {
        started.push(n);
        return Promise.resolve(n);
      },
      undefined,
      () => started.length < 2,
    );
    expect(started).toEqual([1, 2]);
  });

  it("handles an empty list", async () => {
    expect(await runPool([], 4, (x: number) => Promise.resolve(x))).toEqual([]);
  });
});
